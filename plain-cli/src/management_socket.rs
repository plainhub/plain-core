use plain_libs::anyhow::{Result, anyhow, ensure};
use plain_server::native::management::NativeWebLoginRequests;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{path::Path, time::Duration};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

const VERSION: u8 = 1;
const REQUEST_LIMIT: usize = 128 * 1024;
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
    Requests {
        after: Option<String>,
    },
    Decide {
        request_id: String,
        allowed: bool,
    },
    SetPref {
        key: String,
        value: plain_libs::serde_json::Value,
    },
    SetPassword {
        password: String,
    },
    Status {},
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
    Updated,
    Status { status: Status },
    Error { message: String },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Status {
    pub pid: u32,
    pub listeners: ListenerState,
    pub http_port: i32,
    pub https_port: i32,
    pub error: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ListenerState {
    Running,
    Restarting,
    Stopped,
    Failed,
}

pub fn not_running(error: &plain_libs::anyhow::Error) -> bool {
    error.downcast_ref::<std::io::Error>().is_some_and(|error| {
        matches!(
            error.kind(),
            std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
        )
    })
}

pub async fn status(directory: &Path) -> Result<Status> {
    match exchange(directory, Command::Status {}).await? {
        ReplyResult::Status { status } => Ok(status),
        _ => Err(anyhow!("Unexpected local management reply")),
    }
}

pub async fn set_pref(
    directory: &Path,
    key: String,
    value: plain_libs::serde_json::Value,
) -> Result<()> {
    match exchange(directory, Command::SetPref { key, value }).await? {
        ReplyResult::Updated => Ok(()),
        _ => Err(anyhow!("Unexpected local management reply")),
    }
}

pub async fn set_password(directory: &Path, password: String) -> Result<()> {
    match exchange(directory, Command::SetPassword { password }).await? {
        ReplyResult::Updated => Ok(()),
        _ => Err(anyhow!("Unexpected local management reply")),
    }
}

async fn read_frame<T: DeserializeOwned>(
    stream: &mut (impl AsyncRead + Unpin),
    limit: usize,
) -> Result<T> {
    let length = stream.read_u32().await? as usize;
    ensure!(
        length > 0 && length <= limit,
        "Invalid local management frame length"
    );
    let mut bytes = vec![0; length];
    stream.read_exact(&mut bytes).await?;
    plain_libs::serde_json::from_slice(&bytes)
        .map_err(|_| anyhow!("Invalid local management frame"))
}

async fn write_frame<T: Serialize>(
    stream: &mut (impl AsyncWrite + Unpin),
    value: &T,
    limit: usize,
) -> Result<()> {
    let bytes = plain_libs::serde_json::to_vec(value)?;
    ensure!(bytes.len() <= limit, "Local management frame is too large");
    stream.write_u32(bytes.len() as u32).await?;
    stream.write_all(&bytes).await?;
    stream.flush().await?;
    Ok(())
}

pub async fn requests(directory: &Path, after: Option<String>) -> Result<NativeWebLoginRequests> {
    match exchange(directory, Command::Requests { after }).await? {
        ReplyResult::Requests { page } => Ok(page),
        _ => Err(anyhow!("Unexpected local management reply")),
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
        _ => Err(anyhow!("Unexpected local management reply")),
    }
}

pub fn available() -> bool {
    cfg!(any(target_os = "linux", target_os = "macos"))
}

async fn exchange(directory: &Path, command: Command) -> Result<ReplyResult> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        use plain_libs::platform::local_socket;
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
                "Unsupported local management reply version"
            );
            match reply.result {
                ReplyResult::Error { message } => Err(anyhow!(message)),
                value => Ok(value),
            }
        })
        .await
        .map_err(|_| anyhow!("Local management service timed out"))?
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = (directory, command);
        Err(anyhow!("Local management transport is unavailable"))
    }
}

async fn command(
    management: &plain_server::native::management::NativeManagement,
    state: &plain_rs::runtime::Runtime,
    command: Command,
) -> Result<ReplyResult> {
    let id = |value: String| {
        plain_libs::utils::uuid::Uuid::parse_str(&value)
            .map(|value| value.to_string())
            .map_err(|_| anyhow!("Invalid login request id"))
    };
    match command {
        Command::Status {} => {
            let snapshot = state.http_server.snapshot();
            Ok(ReplyResult::Status {
                status: Status {
                    pid: std::process::id(),
                    listeners: if snapshot.restarting {
                        ListenerState::Restarting
                    } else if !snapshot.error.is_empty() {
                        ListenerState::Failed
                    } else if snapshot.http_port != 0 {
                        ListenerState::Running
                    } else {
                        ListenerState::Stopped
                    },
                    http_port: snapshot.http_port,
                    https_port: snapshot.https_port,
                    error: snapshot.error,
                },
            })
        }
        Command::SetPref { key, value } => {
            plain_rs::features::services::settings::preferences::Preferences::from_server(state)
                .set(&key, value)
                .await?;
            if matches!(key.as_str(), "http_port" | "https_port") {
                let result = management
                    .restart_http_server(None, None)
                    .await
                    .map_err(plain_libs::anyhow::Error::msg)?;
                ensure!(
                    result.error.is_empty(),
                    "HTTP listener restart failed: {}",
                    result.error
                );
            }
            Ok(ReplyResult::Updated)
        }
        Command::SetPassword { password } => {
            plain_rs::features::prefs::security::set_password_login(&state.prefs, &password)?;
            let _ = state
                .events
                .send(plain_rs::infrastructure::events::WsEvent::broadcast(
                    "PREFS_UPDATED",
                    "{}".into(),
                ));
            Ok(ReplyResult::Updated)
        }
        Command::Requests { after } => Ok(ReplyResult::Requests {
            page: management
                .web_login_requests(after.map(id).transpose()?)
                .map_err(plain_libs::anyhow::Error::msg)?,
        }),
        Command::Decide {
            request_id,
            allowed,
        } => {
            management
                .confirm_web_login(id(request_id)?, allowed)
                .await
                .map_err(plain_libs::anyhow::Error::msg)?;
            Ok(ReplyResult::Decided)
        }
    }
}

pub(crate) struct Service {
    task: Option<tokio::task::JoinHandle<Result<()>>>,
}

impl Service {
    pub(crate) async fn start(core: &plain_server::server::ContentServer) -> Result<Option<Self>> {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            use plain_libs::platform::local_socket::{self, SocketDirectory};
            use std::os::fd::AsFd;
            let state = core.runtime();
            let directory = SocketDirectory::open(&state.directory)?;
            let path = directory.socket_path();
            if let Some(identity) = directory.existing_socket()? {
                match tokio::time::timeout(DEADLINE, tokio::net::UnixStream::connect(&path)).await {
                    Ok(Err(error)) if error.kind() == std::io::ErrorKind::ConnectionRefused => {
                        directory.remove_stale(identity)?
                    }
                    Ok(Err(error)) => return Err(error.into()),
                    _ => return Err(anyhow!("Local management listener is already running")),
                }
            }
            let listener = tokio::net::UnixListener::bind(&path)?;
            let socket = directory.protect_socket()?;
            let management = core
                .native_management()
                .map_err(plain_libs::anyhow::Error::msg)?;
            let mut stop = state.stop.clone();
            let state = state.clone();
            let task = tokio::spawn(async move {
                let mut workers = tokio::task::JoinSet::new();
                let result = async {
                    while !*stop.borrow() {
                        tokio::select! {
                            biased;
                            _ = stop.changed() => break,
                            result = workers.join_next(), if !workers.is_empty() => {
                                if let Some(result) = result {
                                    result.map_err(|error| anyhow!("Local management worker failed: {error}"))?;
                                }
                            }
                            accepted = listener.accept(), if workers.len() < 16 => {
                                let (stream, _) = accepted?;
                                if let Err(error) = local_socket::peer_uid(stream.as_fd())
                                    .and_then(|uid| local_socket::authorize_uid(uid, local_socket::current_uid()))
                                {
                                    log::warn!("Rejected local management peer: {error}");
                                    continue;
                                }
                                let management = management.clone();
                                let state = state.clone();
                                let mut worker_stop = stop.clone();
                                workers.spawn(async move {
                                    let mut stream = stream;
                                    let handled = async {
                                        let result = async {
                                            let request: Request = read_frame(&mut stream, REQUEST_LIMIT).await?;
                                            ensure!(request.version == VERSION, "Unsupported local management request version");
                                            command(&management, &state, request.command).await
                                        }.await;
                                        let result = result.unwrap_or_else(|error: plain_libs::anyhow::Error| ReplyResult::Error {
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
                                                Ok(Err(error)) => log::debug!("Local management connection failed: {error}"),
                                                Err(_) => log::debug!("Local management connection timed out"),
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
                            log::error!("Local management worker failed during shutdown: {error}");
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
            .ok_or_else(|| anyhow!("Local management service stopped"))?
            .await;
        self.task.take();
        result??;
        Err(anyhow!("Local management service stopped"))
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
#[path = "../../tests/plain-cli/unit/management_socket.rs"]
mod tests;
