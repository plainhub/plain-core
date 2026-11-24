use crate::infrastructure::db::{DChat, Db};
use plain_libs::anyhow::Result;
use std::path::{Path, PathBuf};

pub(crate) enum Source {
    Local(PathBuf),
    #[cfg(feature = "runtime")]
    Staged(plain_libs::temporary::NamedTempFile),
}

impl Source {
    fn path(&self) -> &Path {
        match self {
            Self::Local(path) => path,
            #[cfg(feature = "runtime")]
            Self::Staged(file) => file.path(),
        }
    }
}

pub(crate) async fn import<F: std::future::Future<Output = Result<Source>> + Send>(
    db: &Db,
    directory: &Path,
    mut chat: DChat,
    source: impl Fn(String) -> F + Send,
) -> Result<DChat> {
    use crate::features::chat::attachments::app_file_store;
    use plain_libs::serde_json::{self, Value};

    let content: Value = serde_json::from_str(&chat.content)?;
    let Some(items) = content["value"]["items"]
        .as_array()
        .filter(|_| content["type"] == "MESSAGE")
    else {
        return Ok(chat);
    };
    for item in items {
        let attachment = &item["attachment"];
        let field = |name| {
            attachment[name]
                .as_str()
                .ok_or_else(|| plain_libs::anyhow::anyhow!("Missing attachment {name}"))
        };
        let uri = field("uri")?;
        if uri.starts_with("fid:") {
            continue;
        }
        let id = field("id")?;
        let name = attachment["fileName"].as_str().unwrap_or_default().to_owned();
        let file = source(uri.to_owned()).await?;
        let db = db.clone();
        let directory = directory.to_path_buf();
        let (message_id, id, uri) = (chat.id.clone(), id.to_owned(), uri.to_owned());
        let stored = tokio::task::spawn_blocking(move || {
            app_file_store::import_attachment(
                &db,
                &directory,
                file.path(),
                &name,
                &message_id,
                &id,
                &uri,
            )
        })
        .await??;
        chat = stored
            .chat
            .ok_or_else(|| plain_libs::anyhow::anyhow!("Missing imported attachment message"))?;
    }
    Ok(chat)
}
