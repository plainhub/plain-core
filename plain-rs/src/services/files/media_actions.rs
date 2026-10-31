use anyhow::Result;
use plain_rs::db::Db;
use plain_rs::enums::DataType;
use plain_rs::host::bridge::Host;
use plain_rs::media::actions as media_actions;
use plain_rs::media::actions::Action;
use plain_rs::media::actions::Outcome;
use plain_rs::media::audio::commands as audio_commands;
use plain_rs::media::audio::playback as audio_playback;
use plain_rs::prefs::Prefs;
use plain_rs::services::audio::host::Audio;
use plain_rs::services::images::index::ImageIndex;
use std::sync::Arc;
pub struct MediaActions {
    #[cfg(feature = "runtime-media")]
    scans: Arc<plain_rs::services::files::media_scans::MediaScans>,
    platform: Arc<plain_rs::services::files::platform::FilePlatform>,
    duration_cache: Arc<plain_rs::media::duration_cache::DurationCache>,
    db: Arc<Db>,
    host: Arc<Host>,
    audio: Arc<Audio>,
    index: Arc<ImageIndex>,
    prefs: Arc<Prefs>,
    events: tokio::sync::broadcast::Sender<plain_rs::events::WsEvent>,
}
pub struct Completed {
    pub affected_count: i32,
    pub failed_ids: Vec<String>,
}
impl MediaActions {
    pub fn new(
        #[cfg(feature = "runtime-media")] scans: Arc<
            plain_rs::services::files::media_scans::MediaScans,
        >,
        platform: Arc<plain_rs::services::files::platform::FilePlatform>,
        db: Arc<Db>,
        host: Arc<Host>,
        audio: Arc<Audio>,
        index: Arc<ImageIndex>,
        prefs: Arc<Prefs>,
        events: tokio::sync::broadcast::Sender<plain_rs::events::WsEvent>,
        duration_cache: Arc<plain_rs::media::duration_cache::DurationCache>,
    ) -> Self {
        Self {
            #[cfg(feature = "runtime-media")]
            scans,
            platform,
            duration_cache,
            db,
            host,
            audio,
            index,
            prefs,
            events,
        }
    }
    pub async fn run_query(
        &self,
        kind: DataType,
        action: Action,
        query: &str,
        dest_dir: &str,
    ) -> Result<i32> {
        let capabilities = plain_rs::services::files::media_action_io::capability_list(&self.host)
            .await
            .map_err(anyhow::Error::msg)?;
        plain_rs::services::capabilities::require_list(
            &capabilities,
            plain_rs::services::files::media_action_io::action_capability(action),
        )
        .map_err(|error| anyhow::Error::msg(error.to_string()))?;
        let from_trash = action == Action::Restore
            || (action == Action::Delete
                && capabilities.iter().any(|value| value == "MEDIA_TRASH"));
        if !matches!(
            kind,
            DataType::Audio | DataType::Video | DataType::Image | DataType::Doc
        ) {
            anyhow::bail!("unsupported media kind");
        }
        let ids = plain_rs::services::files::catalog::MediaCatalog::new(
            self.db.clone(),
            self.host.clone(),
        )
        .ids(kind, query, from_trash)
        .await
        .map_err(anyhow::Error::msg)?;
        let result = self
            .run_batches(kind, action, ids, from_trash, dest_dir.to_owned())
            .await?;
        let affected = result.affected_count;
        let failed = result.failed_ids.len();
        if failed != 0 {
            return Err(anyhow::anyhow!(
                "{}: {failed} media actions failed ({affected} completed)",
                format!("{action:?}").to_uppercase()
            ));
        }
        Ok(affected)
    }
    async fn run_batches(
        &self,
        kind: DataType,
        action: Action,
        requested: Vec<String>,
        from_trash: bool,
        dest_dir: String,
    ) -> Result<Completed> {
        if !matches!(
            kind,
            DataType::Audio | DataType::Video | DataType::Image | DataType::Doc
        ) {
            anyhow::bail!("unsupported media kind");
        }
        if requested.iter().any(String::is_empty) {
            anyhow::bail!("invalid media selection");
        }
        let mut seen = std::collections::HashSet::new();
        let ids = requested
            .into_iter()
            .filter(|id| seen.insert(id.clone()))
            .collect::<Vec<_>>();
        let mut completed = Completed {
            affected_count: 0,
            failed_ids: vec![],
        };
        for batch in ids.chunks(500) {
            let result = self
                .apply(kind, action, batch.to_vec(), from_trash, dest_dir.clone())
                .await?;
            completed.affected_count = completed
                .affected_count
                .checked_add(result.affected_count)
                .ok_or_else(|| anyhow::Error::msg("Media action count overflow"))?;
            completed.failed_ids.extend(result.failed_ids);
        }
        Ok(completed)
    }

    async fn apply(
        &self,
        r#type: DataType,
        action: Action,
        requested: Vec<String>,
        from_trash: bool,
        dest_dir: String,
    ) -> Result<Completed> {
        let invalid = Outcome {
            successful: Vec::new(),
            failed_ids: requested.clone(),
        };
        media_actions::validate(r#type, action, &requested, &invalid)?;
        if requested.is_empty() {
            return Ok(Completed {
                affected_count: 0,
                failed_ids: Vec::new(),
            });
        }
        if action == Action::Move && dest_dir.is_empty() {
            anyhow::bail!("media destination required");
        }
        let operation = plain_rs::services::files::media_action_io::Operation::prepare(
            &self.host,
            &self.platform,
            r#type,
            action,
            requested,
            from_trash,
            dest_dir,
            #[cfg(feature = "runtime-media")]
            self.scans.clone(),
        )
        .await
        .map_err(anyhow::Error::msg)?;
        let result = if r#type == DataType::Audio {
            let prefs = self.prefs.clone();
            self.audio
                .run(move |db, engine| {
                    let before = audio_playback::snapshot(db)?;
                    operation.complete(db, |outcome| {
                        let count =
                            media_actions::cleanup(db, r#type, action, &outcome.successful)?;
                        if action != Action::Restore
                            && !before.path.is_empty()
                            && outcome
                                .successful
                                .iter()
                                .any(|item| item.path == before.path)
                        {
                            if action == Action::Move {
                                audio_commands::clear_engine(db, &prefs, engine)?;
                            } else {
                                audio_commands::command(
                                    db,
                                    &prefs,
                                    engine,
                                    audio_commands::Action::Clear,
                                    0,
                                    1.0,
                                )?;
                            }
                        }
                        Ok((count, outcome))
                    })
                })
                .await?
        } else {
            let db = self.db.clone();
            let index = self.index.clone();
            tokio::task::spawn_blocking(move || {
                operation.complete(&db, |outcome| {
                    if outcome.successful.is_empty() {
                        return Ok((0, outcome));
                    }
                    let count = if r#type == DataType::Image && action != Action::Restore {
                        index.update_cache(|db| {
                            media_actions::cleanup(db, r#type, action, &outcome.successful)
                        })?
                    } else {
                        media_actions::cleanup(&db, r#type, action, &outcome.successful)?
                    };
                    Ok((count, outcome))
                })
            })
            .await??
        };
        let (affected, outcome) = result;
        let completed_ids: Vec<_> = outcome
            .successful
            .iter()
            .map(|item| item.id.clone())
            .collect();
        if matches!(r#type, DataType::Audio | DataType::Video) {
            self.duration_cache
                .remove(r#type.media_type_str().unwrap(), &completed_ids)?;
        }
        if !completed_ids.is_empty() {
            plain_rs::services::files::catalog_events::publish(&self.events, [r#type.kind()]);
            let _ = self.events.send(plain_rs::events::WsEvent::broadcast(
                "CONTENT_CHANGED",
                "{}".into(),
            ));
        }
        Ok(Completed {
            affected_count: affected.try_into()?,
            failed_ids: outcome.failed_ids,
        })
    }
}
