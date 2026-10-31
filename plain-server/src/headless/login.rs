use crate::native::management::NativeWebLoginRequests;
use anyhow::{Result, anyhow, ensure};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{path::Path, time::Duration};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

const VERSION: u8 = 1;
const REQUEST_LIMIT: usize = 4096;
const REPLY_LIMIT: usize = 1024 * 1024;
const DEADLINE: Duration = Duration::from_secs(3);

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    version: u8,
    command: Command,
}

#[derive(Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum Command {
    Requests { after: Option<String> },
    Decide { request_id: String, allowed: bool },
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Reply {
    version: u8,
    result: ReplyResult,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
enum ReplyResult {
    Requests { page: NativeWebLoginRequests },
    Decided,
    Error { message: String },
}

async fn read_frame<T: DeserializeOwned>(
    stream: &mut (impl AsyncRead + Unpin),
    limit: usize,
) -> Result<T> {
    let length = stream.read_u32().await? as usize;
    ensure!(
        length > 0 && length <= limit,
        "Invalid local approval frame length"
    );
    let mut bytes = vec![0; length];
    stream.read_exact(&mut bytes).await?;
    serde_json::from_slice(&bytes).map_err(|_| anyhow!("Invalid local approval frame"))
}

async fn write_frame<T: Serialize>(
    stream: &mut (impl AsyncWrite + Unpin),
    value: &T,
    limit: usize,
) -> Result<()> {
    let bytes = serde_json::to_vec(value)?;
    ensure!(bytes.len() <= limit, "Local approval frame is too large");
    stream.write_u32(bytes.len() as u32).await?;
    stream.write_all(&bytes).await?;
    stream.flush().await?;
    Ok(())
}

pub async fn requests(directory: &Path, after: Option<String>) -> Result<NativeWebLoginRequests> {
    match exchange(directory, Command::Requests { after }).await? {
        ReplyResult::Requests { page } => Ok(page),
        _ => Err(anyhow!("Unexpected local approval reply")),
    }
}

pub async fn decide(directory: &Path, request_id: String, allowed: bool) -> Result<()> {
    match exchange(
        directory,
        Command::Decide {
            request_id,
            allowed,
        },
    )
    .await?
    {
        ReplyResult::Decided => Ok(()),
        _ => Err(anyhow!("Unexpected local approval reply")),
    }
}

pub fn available() -> bool {
    cfg!(any(target_os = "linux", target_os = "macos"))
}

async fn exchange(directory: &Path, command: Command) -> Result<ReplyResult> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        use plain_rs::platform::local_socket;
        use std::os::fd::AsFd;
        let (path, owner) = local_socket::client_path(directory)?;
        tokio::time::timeout(DEADLINE, async {
            let mut stream = tokio::net::UnixStream::connect(path).await?;
            local_socket::authorize_uid(local_socket::peer_uid(stream.as_fd())?, owner)?;
            write_frame(
                &mut stream,
                &Request {
                    version: VERSION,
                    command,
                },
                REQUEST_LIMIT,
            )
            .await?;
            let reply: Reply = read_frame(&mut stream, REPLY_LIMIT).await?;
            ensure!(
                reply.version == VERSION,
                "Unsupported local approval reply version"
            );
            match reply.result {
                ReplyResult::Error { message } => Err(anyhow!(message)),
                value => Ok(value),
            }
        })
        .await
        .map_err(|_| anyhow!("Local approval service timed out"))?
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = (directory, command);
        Err(anyhow!("Local approval transport is unavailable"))
    }
}

async fn command(
    management: &crate::native::management::NativeManagement,
    command: Command,
) -> Result<ReplyResult> {
    let id = |value: String| {
        uuid::Uuid::parse_str(&value)
            .map(|value| value.to_string())
            .map_err(|_| anyhow!("Invalid login request id"))
    };
    match command {
        Command::Requests { after } => Ok(ReplyResult::Requests {
            page: management
                .web_login_requests(after.map(id).transpose()?)
                .map_err(anyhow::Error::msg)?,
        }),
        Command::Decide {
            request_id,
            allowed,
        } => {
            management
                .confirm_web_login(id(request_id)?, allowed)
                .await
                .map_err(anyhow::Error::msg)?;
            Ok(ReplyResult::Decided)
        }
    }
}

pub(crate) struct Service {
    task: Option<tokio::task::JoinHandle<Result<()>>>,
}

impl Service {
    pub(crate) async fn start(core: &crate::server::ContentServer) -> Result<Option<Self>> {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            use plain_rs::platform::local_socket::{self, SocketDirectory};
            use std::os::fd::AsFd;
            let state = core.headless_state();
            let directory = SocketDirectory::open(&state.directory)?;
            let path = directory.socket_path();
            if let Some(identity) = directory.existing_socket()? {
                match tokio::time::timeout(DEADLINE, tokio::net::UnixStream::connect(&path)).await {
                    Ok(Err(error)) if error.kind() == std::io::ErrorKind::ConnectionRefused => {
                        directory.remove_stale(identity)?
                    }
                    Ok(Err(error)) => return Err(error.into()),
                    _ => return Err(anyhow!("Local approval listener is already running")),
                }
            }
            let listener = tokio::net::UnixListener::bind(&path)?;
            let socket = directory.protect_socket()?;
            let management = core.native_management().map_err(anyhow::Error::msg)?;
            let mut stop = state.stop.clone();
            let task = tokio::spawn(async move {
                let mut workers = tokio::task::JoinSet::new();
                let result = async {
                    while !*stop.borrow() {
                        tokio::select! {
                            biased;
                            _ = stop.changed() => break,
                            result = workers.join_next(), if !workers.is_empty() => {
                                if let Some(result) = result {
                                    result.map_err(|error| anyhow!("Local approval worker failed: {error}"))?;
                                }
                            }
                            accepted = listener.accept(), if workers.len() < 16 => {
                                let (stream, _) = accepted?;
                                if let Err(error) = local_socket::peer_uid(stream.as_fd())
                                    .and_then(|uid| local_socket::authorize_uid(uid, local_socket::current_uid()))
                                {
                                    log::warn!("Rejected local approval peer: {error}");
                                    continue;
                                }
                                let management = management.clone();
                                let mut worker_stop = stop.clone();
                                workers.spawn(async move {
                                    let mut stream = stream;
                                    let handled = async {
                                        let result = async {
                                            let request: Request = read_frame(&mut stream, REQUEST_LIMIT).await?;
                                            ensure!(request.version == VERSION, "Unsupported local approval request version");
                                            command(&management, request.command).await
                                        }.await;
                                        let result = result.unwrap_or_else(|error: anyhow::Error| ReplyResult::Error {
                                            message: error.to_string().chars().take(512).collect(),
                                        });
                                        write_frame(&mut stream, &Reply { version: VERSION, result }, REPLY_LIMIT).await
                                    };
                                    tokio::select! {
                                        biased;
                                        _ = worker_stop.wait_for(|stopped| *stopped) => {},
                                        result = tokio::time::timeout(DEADLINE, handled) => {
                                            match result {
                                                Ok(Ok(())) => {},
                                                Ok(Err(error)) => log::debug!("Local approval connection failed: {error}"),
                                                Err(_) => log::debug!("Local approval connection timed out"),
                                            }
                                        }
                                    }
                                });
                            }
                        }
                    }
                    Ok(())
                }.await;
                workers.abort_all();
                while let Some(result) = workers.join_next().await {
                    if let Err(error) = result {
                        if !error.is_cancelled() {
                            log::error!("Local approval worker failed during shutdown: {error}");
                        }
                    }
                }
                drop(listener);
                drop(socket);
                drop(directory);
                result
            });
            Ok(Some(Self { task: Some(task) }))
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let _ = core;
            Ok(None)
        }
    }

    pub(crate) async fn wait(&mut self) -> Result<()> {
        let result = self
            .task
            .as_mut()
            .ok_or_else(|| anyhow!("Local approval service stopped"))?
            .await;
        self.task.take();
        result??;
        Err(anyhow!("Local approval service stopped"))
    }

    pub(crate) async fn shutdown(mut self) -> Result<()> {
        if let Some(task) = self.task.take() {
            task.await??;
        }
        Ok(())
    }
}

pub(crate) async fn wait(service: &mut Option<Service>) -> Result<()> {
    match service {
        Some(service) => service.wait().await,
        None => std::future::pending().await,
    }
}

#[cfg(test)]
#[path = "../../tests/unit/headless/login.rs"]
mod tests;
