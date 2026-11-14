use std::{collections::HashMap, net::IpAddr, sync::Mutex, time::Duration};
use tokio::{sync::watch, time::Instant};

const TTL: Duration = Duration::from_secs(30 * 60);

#[derive(Clone)]
pub struct Resource {
    pub path: String,
    pub mime: String,
    lease: watch::Receiver<Option<Instant>>,
}

impl Resource {
    pub async fn canceled(self) {
        canceled(self.lease).await;
    }
}

async fn canceled(mut lease: watch::Receiver<Option<Instant>>) {
    let mut deadline = *lease.borrow_and_update();
    while let Some(expires_at) = deadline {
        tokio::select! {
            biased;
            result = lease.changed() => if result.is_err() { return; },
            _ = tokio::time::sleep_until(expires_at) => return,
        }
        deadline = *lease.borrow_and_update();
    }
}

pub struct Session {
    peer: IpAddr,
    resources: HashMap<String, Resource>,
    lease: watch::Sender<Option<Instant>>,
}

impl Session {
    pub fn new(peer: IpAddr) -> Self {
        Self {
            peer: peer.to_canonical(),
            resources: HashMap::new(),
            lease: watch::channel(Some(Instant::now() + TTL)).0,
        }
    }

    pub fn pending(&self) -> PendingSession {
        PendingSession(Some(self.lease.clone()))
    }

    pub fn register(&mut self, path: &str, mime: &str) -> String {
        let mut random = [0; 32];
        plain_libs::crypto::gen_random(&mut random);
        let token = plain_libs::utils::hex::bytes_to_hex(&random);
        let ext = extension(path, mime);
        let name = format!("{token}.{ext}");
        self.resources.insert(
            name.clone(),
            Resource {
                path: path.to_owned(),
                mime: mime.trim().to_owned(),
                lease: self.lease.subscribe(),
            },
        );
        name
    }
}

pub struct PendingSession(Option<watch::Sender<Option<Instant>>>);

impl PendingSession {
    pub fn canceled(&self) -> impl std::future::Future<Output = ()> + Send + 'static {
        canceled(self.0.as_ref().unwrap().subscribe())
    }

    pub fn keep(mut self) {
        self.0.take();
    }
}

impl Drop for PendingSession {
    fn drop(&mut self) {
        if let Some(lease) = self.0.take() {
            lease.send_replace(None);
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.lease.send_replace(None);
    }
}

#[derive(Default)]
struct Current {
    generation: u64,
    interruption: u64,
    session: Option<Session>,
}

#[derive(Default)]
pub struct Registry(Mutex<Current>);

impl Registry {
    pub fn interruption(&self) -> u64 {
        self.0.lock().unwrap().interruption
    }

    pub fn interrupt(&self) {
        let mut current = self.0.lock().unwrap();
        current.interruption = current.interruption.wrapping_add(1);
        current.generation = current.generation.wrapping_add(1);
        current.session.take();
    }

    pub fn generation(&self) -> u64 {
        self.0.lock().unwrap().generation
    }

    pub fn replace_current(&self, session: Session, generation: u64) -> bool {
        let mut current = self.0.lock().unwrap();
        if current.generation != generation {
            return false;
        }
        current.generation = current.generation.wrapping_add(1);
        current.session = Some(session);
        true
    }

    pub fn replace(&self, session: Session) {
        let mut current = self.0.lock().unwrap();
        current.generation = current.generation.wrapping_add(1);
        current.session = Some(session);
    }

    pub fn revoke(&self) -> u64 {
        let mut current = self.0.lock().unwrap();
        current.generation = current.generation.wrapping_add(1);
        current.session.take();
        current.generation
    }

    pub fn renew(&self) {
        let guard = self.0.lock().unwrap();
        if let Some(session) = guard.session.as_ref().filter(|session| valid(session)) {
            session.lease.send_replace(Some(Instant::now() + TTL));
        }
    }

    pub fn active(&self) -> bool {
        self.0.lock().unwrap().session.as_ref().is_some_and(valid)
    }

    pub fn accepts(&self, peer: IpAddr) -> bool {
        self.0
            .lock()
            .unwrap()
            .session
            .as_ref()
            .is_some_and(|session| valid(session) && session.peer == peer.to_canonical())
    }

    pub fn lookup(&self, name: &str, peer: IpAddr) -> Option<Resource> {
        let guard = self.0.lock().unwrap();
        let session = guard.session.as_ref()?;
        if !valid(session) || session.peer != peer.to_canonical() {
            return None;
        }
        session.resources.get(name).cloned()
    }
}

fn valid(session: &Session) -> bool {
    session
        .lease
        .borrow()
        .is_some_and(|deadline| deadline > Instant::now())
}

fn extension(path: &str, mime: &str) -> String {
    let path = path.split(['?', '#']).next().unwrap_or_default();
    path.rsplit('/')
        .next()
        .unwrap_or_default()
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .filter(|ext| {
            !ext.is_empty() && ext.len() <= 16 && ext.bytes().all(|b| b.is_ascii_alphanumeric())
        })
        .unwrap_or_else(|| {
            match mime.split(';').next().unwrap_or_default().trim() {
                "video/mp4" | "audio/mp4" => "mp4",
                "video/x-matroska" => "mkv",
                "video/webm" => "webm",
                "video/mp2t" => "ts",
                "audio/mpeg" => "mp3",
                "audio/flac" => "flac",
                "audio/wav" | "audio/x-wav" => "wav",
                "audio/aac" => "aac",
                "audio/ogg" => "ogg",
                "image/jpeg" => "jpg",
                "image/png" => "png",
                "image/webp" => "webp",
                _ => "bin",
            }
            .to_owned()
        })
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/casting/media_alias.rs"]
mod tests;
