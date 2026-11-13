use crate::features::downloads::{Source, queue::Run};
use crate::runtime::Runtime as CoreRuntime;
use plain_libs::anyhow::{Result, ensure};
use std::sync::Arc;

mod selection;
pub(super) use selection::files;
pub use selection::received;

struct Import {
    imports: Arc<crate::features::chat::attachments::attachment_imports::Imports>,
    token: String,
}
impl Drop for Import {
    fn drop(&mut self) {
        if let Err(error) = self.imports.abort(&self.token) {
            log::error!("Attachment cleanup failed: {error}");
        }
    }
}

pub(super) async fn execute(
    state: &CoreRuntime,
    run: &Run,
    work: &super::io::BlockingWork,
) -> Result<()> {
    let Source::Chat {
        message_id,
        peer_id,
        file_ids,
    } = &run.download.source
    else {
        unreachable!()
    };
    let entries = files(state, message_id, peer_id, file_ids, &run.completed)?;
    for file in &entries {
        if run.completed.contains_key(&file.id) {
            continue;
        }
        let expected = run
            .attachments
            .iter()
            .find(|expected| expected.id == file.id)
            .expect("Selected attachment was bound at creation");
        ensure!(
            file.uri == expected.uri && file.size_bytes == expected.size_bytes,
            "Attachment changed"
        );
    }
    let total = entries.iter().try_fold(0_i64, |total, file| {
        total
            .checked_add(file.size_bytes)
            .ok_or_else(|| plain_libs::anyhow::anyhow!("Attachment size overflow"))
    })?;
    state.downloads.plan(
        &run.download.id,
        &run.generation,
        i32::try_from(entries.len())?,
        total,
        &run.download.title,
    )?;
    for file in entries {
        let id = &file.id;
        if run.completed.contains_key(id) {
            continue;
        }
        let size = file.size_bytes;
        let name = &file.name;
        state
            .downloads
            .begin_file(&run.download.id, &run.generation, name, size)?;
        let result = async {
            let peer = run.peers[0].clone();
            let uri = &file.uri;
            let ticket =
                state
                    .attachments
                    .begin(&state.db, &state.directory, message_id, id, uri)?;
            let _import = Import {
                imports: state.attachments.clone(),
                token: ticket.token.clone(),
            };
            let task = crate::features::services::peers::peer_download::Task {
                id: run.download.id.clone(),
                file_id: file
                    .uri
                    .strip_prefix("fsid:")
                    .expect("Remote attachment validated")
                    .into(),
                peer,
                total: u64::try_from(size)?,
                generation: run.generation.clone(),
            };
            crate::features::services::peers::peer_download::run(
                state,
                &task,
                &ticket.path,
                run.attempt,
            )
            .await?;
            let owner = state.clone();
            let download_id = run.download.id.clone();
            let generation = run.generation.clone();
            let file_id = id.to_owned();
            let message_id = message_id.clone();
            work.run(move || -> Result<()> {
                owner
                    .downloads
                    .finish_file(&download_id, &generation, &file_id, || {
                        owner
                            .attachments
                            .finish(&owner.db, &owner.directory, &ticket.token)
                            .map(|_| ())
                    })?;
                let row =
                    crate::infrastructure::db::chat_store::messages::get(&owner.db, &message_id)?
                        .ok_or_else(|| plain_libs::anyhow::anyhow!("Message unavailable"))?;
                let _ = owner
                    .events
                    .send(crate::infrastructure::events::WsEvent::broadcast(
                        crate::features::chat::events::WS_MESSAGE_UPDATED,
                        plain_libs::serde_json::json!([
                            crate::features::chat::service::chat_to_json(&row)
                        ])
                        .to_string(),
                    ));
                Ok(())
            })
            .await?;
            Ok(())
        }
        .await;
        if let Err(error) = result {
            state
                .downloads
                .fail_file(&run.download.id, &run.generation, id, error)?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/downloads/chat.rs"]
mod tests;
