use crate::host::bridge::Host;
use plain_rs::db::{Db, audio_catalog::AudioMedia};
use plain_rs::error::DomainError;
use plain_rs::error::DomainResult;
use plain_rs::media::audio::queue::AudioTrack;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::sync::Arc;

pub struct HostLibrary {
    host: Arc<Host>,
    db: Arc<Db>,
    runtime: tokio::runtime::Handle,
    resources: Arc<crate::host::resources::NativeResources>,
    cache: std::path::PathBuf,
}
impl HostLibrary {
    pub fn new(
        host: Arc<Host>,
        db: Arc<Db>,
        resources: Arc<crate::host::resources::NativeResources>,
        cache: std::path::PathBuf,
    ) -> Self {
        Self {
            host,
            db,
            runtime: tokio::runtime::Handle::current(),
            resources,
            cache,
        }
    }
    pub fn refresh(&mut self) -> DomainResult<()> {
        if self.host.uses_system_media_catalog() {
            self.db.use_platform_audio_catalog()?;
            let mut ids = self.db.audio_reference_ids()?;
            for id in self.db.audio_selected_ids() {
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
            let rows: Vec<AudioMedia> = if ids.is_empty() {
                Vec::new()
            } else {
                self.resolve(&ids, &[])?
            };
            self.db.cache_platform_audio(&rows, true)?;
        }
        self.db.restore_audio_current()?;
        Ok(())
    }

    pub fn restore_current(&mut self) -> DomainResult<()> {
        if self.host.uses_system_media_catalog() {
            self.db.use_platform_audio_catalog()?;
            if self.db.audio_restore_needed() {
                let id = self.db.audio_current_file_id();
                if let Some(id) = id.filter(|id| !id.is_empty()) {
                    let rows: Vec<AudioMedia> = self.resolve(&[id], &[])?;
                    self.db.cache_platform_audio(&rows, false)?;
                }
            }
        }
        self.db.restore_audio_current()?;
        Ok(())
    }

    pub fn cache_paths(&mut self, paths: &[String]) -> DomainResult<()> {
        if self.host.uses_system_media_catalog() && !paths.is_empty() {
            self.db.forget_platform_audio_paths(paths)?;
            let rows: Vec<AudioMedia> = self.resolve(&[], paths)?;
            self.db.cache_platform_audio(&rows, false)?;
        }
        Ok(())
    }

    pub(crate) fn tracks_by_ids(&mut self, ids: &[String]) -> DomainResult<Vec<AudioTrack>> {
        self.cache_ids(ids)?;
        ids.iter()
            .map(|id| plain_rs::media::audio::queue::track_by_id(&self.db, id))
            .collect()
    }

    pub fn cache_ids(&mut self, ids: &[String]) -> DomainResult<()> {
        if self.host.uses_system_media_catalog() && !ids.is_empty() {
            let rows: Vec<AudioMedia> = self.resolve(ids, &[])?;
            self.db.cache_platform_audio(&rows, false)?;
        }
        Ok(())
    }

    fn resolve(&self, ids: &[String], paths: &[String]) -> DomainResult<Vec<AudioMedia>> {
        let mut rows: Vec<AudioMedia> =
            self.call("audioLibraryResolve", json!({"ids":ids,"paths":paths}))?;
        for row in &mut rows {
            row.track.artist =
                plain_rs::media::audio::metadata::normalize_artist(&row.track.artist);
        }
        Ok(rows)
    }

    fn call<T: DeserializeOwned>(&self, method: &str, params: Value) -> DomainResult<T> {
        let result = self
            .runtime
            .block_on(self.host.call(method, params))
            .map_err(DomainError::Other)?;
        serde_json::from_value(result)
            .map_err(|e| DomainError::Other(format!("invalid audio provider response: {e}")))
    }

    fn read_track(&self, path: &str) -> DomainResult<AudioTrack> {
        if path.starts_with("content://") || path.starts_with("ph://") {
            self.runtime.block_on(super::metadata::read_uri(
                &self.resources,
                &self.cache,
                path,
            ))
        } else {
            super::metadata::read_file(path)
        }
    }
}
impl plain_rs::media::audio::commands::Engine for HostLibrary {
    fn metadata(&mut self, path: &str) -> DomainResult<AudioTrack> {
        if path.trim().is_empty() {
            return Err(DomainError::Other("Audio path required".into()));
        }
        self.cache_paths(&[path.to_owned()])?;
        let indexed = self
            .db
            .with_conn(|conn| plain_rs::media::audio::queue::track_by_path_conn(conn, path))?;
        let mut track = if let Some(mut track) = indexed {
            if track.title.is_empty() && track.artist.is_empty() && track.duration_ms == 0 {
                let metadata = self.read_track(path)?;
                track.title = metadata.title;
                track.artist = metadata.artist;
                track.duration_ms = metadata.duration_ms;
                self.db.update_platform_audio_metadata(&track)?;
            }
            track
        } else {
            self.read_track(path)?
        };
        if track.title.is_empty() {
            track.title = AudioTrack::from_path_stem(path).title;
        }
        track.path = path.to_owned();
        Ok(track)
    }
    fn execute(
        &mut self,
        command: &plain_rs::media::audio::commands::EngineCommand,
    ) -> DomainResult<plain_rs::media::audio::commands::EngineReport> {
        self.call(
            "audioEngineCommand",
            serde_json::to_value(command).map_err(|e| DomainError::Other(e.to_string()))?,
        )
    }
}
