use plain_rs::db::Db;
use plain_rs::error::DomainResult;
use plain_rs::host::bridge::Host;
use plain_rs::services::audio::library::HostLibrary;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use tokio::sync::{Mutex, Semaphore};

#[derive(Clone)]
pub struct Audio {
    local: bool,
    db: Arc<Db>,
    host: Arc<Host>,
    lock: Arc<Mutex<()>>,
    restored_host: Arc<AtomicU64>,
    capacity: Arc<Semaphore>,
    resources: Arc<plain_rs::host::resources::NativeResources>,
    cache: std::path::PathBuf,
}
enum Preparation {
    References,
    Current,
    None,
}
impl Audio {
    pub fn new(
        db: Arc<Db>,
        host: Arc<Host>,
        resources: Arc<plain_rs::host::resources::NativeResources>,
        cache: std::path::PathBuf,
    ) -> Self {
        Self {
            local: true,
            db,
            host,
            lock: Arc::new(Mutex::new(())),
            restored_host: Arc::new(AtomicU64::new(0)),
            capacity: Arc::new(Semaphore::new(32)),
            resources,
            cache,
        }
    }
    pub fn remote(&self) -> Self {
        Self {
            local: false,
            db: Arc::new(self.db.remote_audio()),
            ..self.clone()
        }
    }

    pub async fn run<T: Send + 'static>(
        &self,
        operation: impl FnOnce(&Db, &mut HostLibrary) -> DomainResult<T> + Send + 'static,
    ) -> plain_rs::error::DomainResult<T> {
        self.execute(Preparation::References, operation).await
    }

    pub async fn run_playback<T: Send + 'static>(
        &self,
        operation: impl FnOnce(&Db, &mut HostLibrary) -> DomainResult<T> + Send + 'static,
    ) -> plain_rs::error::DomainResult<T> {
        self.execute(Preparation::Current, operation).await
    }

    pub async fn metadata(
        &self,
        path: String,
    ) -> plain_rs::error::DomainResult<plain_rs::media::audio::queue::AudioTrack> {
        self.execute(Preparation::None, move |_, library| {
            plain_rs::media::audio::commands::Engine::metadata(library, &path)
        })
        .await
    }

    pub async fn cover(&self, path: &str) -> Result<Option<std::path::PathBuf>, String> {
        plain_rs::services::audio::covers::cache(&self.resources, &self.cache, path).await
    }

    async fn execute<T: Send + 'static>(
        &self,
        preparation: Preparation,
        operation: impl FnOnce(&Db, &mut HostLibrary) -> DomainResult<T> + Send + 'static,
    ) -> plain_rs::error::DomainResult<T> {
        let db = self.db.clone();
        let permit = self.capacity.clone().try_acquire_owned().map_err(|_| {
            plain_rs::error::DomainError::Other("audio request capacity exceeded".into())
        })?;
        let guard = self.lock.clone().lock_owned().await;
        let mut library = HostLibrary::new(
            self.host.clone(),
            db.clone(),
            self.resources.clone(),
            self.cache.clone(),
        );
        let host = self.host.clone();
        let local = self.local;
        let restored_host = self.restored_host.clone();
        tokio::task::spawn_blocking(move || {
            let generation = host.generation();
            let _permit = permit;
            let _guard = guard;
            let uses_playback = !matches!(preparation, Preparation::None);
            match preparation {
                Preparation::References => library.refresh()?,
                Preparation::Current => library.restore_current()?,
                Preparation::None => {}
            }
            if local
                && uses_playback
                && let Some(generation) = generation
            {
                if !host.is_current(generation) {
                    return Err(plain_rs::error::DomainError::Other(
                        "host disconnected".into(),
                    ));
                }
                if restored_host.load(Ordering::Acquire) != generation {
                    plain_rs::media::audio::playback::invalidate(&db)?;
                    restored_host.store(generation, Ordering::Release);
                }
            }
            operation(&db, &mut library)
        })
        .await
        .map_err(|e| plain_rs::error::DomainError::Other(e.to_string()))?
        .map_err(|e| plain_rs::error::DomainError::Other(e.to_string()))
    }
}
