use plain_libs::axum::http::StatusCode;
use plain_rs::infrastructure::db::{Db, SessionRow};
use plain_rs::infrastructure::transport::graphql_request::{Envelope, RequestId};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
    time::{Duration, Instant},
};

const WINDOW_MS: u64 = 30 * 60 * 1000;
const RETENTION: Duration = Duration::from_secs(60 * 60);
const CLEANUP_INTERVAL: Duration = Duration::from_secs(60);
const SESSION_LIMIT: usize = 65_536;
const TOTAL_LIMIT: usize = 262_144;

type SessionKey = [u8; 32];

pub(crate) struct GraphqlReplay {
    retired: HashSet<SessionKey>,
    state: Mutex<Cache>,
}

struct Cache {
    sessions: HashMap<SessionKey, HashMap<RequestId, Instant>>,
    total: usize,
    last_cleanup: Instant,
    clock_wall: i64,
    clock_instant: Instant,
}

fn session_key(session: &SessionRow) -> SessionKey {
    let mut hash = Sha256::new();
    hash.update((session.client_id.len() as u64).to_be_bytes());
    hash.update(session.client_id.as_bytes());
    hash.update(session.token.as_bytes());
    hash.finalize().into()
}

impl GraphqlReplay {
    pub(crate) fn new(db: &Db) -> Result<Self, String> {
        let retired = db
            .session_list()
            .map_err(|error| error.to_string())?
            .iter()
            .map(session_key)
            .collect();
        let now = Instant::now();
        Ok(Self {
            retired,
            state: Mutex::new(Cache {
                sessions: HashMap::new(),
                total: 0,
                last_cleanup: now,
                clock_wall: plain_libs::chrono::Utc::now().timestamp_millis(),
                clock_instant: now,
            }),
        })
    }

    pub(crate) fn admit(
        &self,
        db: &Db,
        session: &SessionRow,
        plain: &str,
    ) -> Result<(), StatusCode> {
        let key = session_key(session);
        if self.retired.contains(&key) {
            return Err(StatusCode::UNAUTHORIZED);
        }
        let id = Envelope::parse(plain)
            .map_err(|_| StatusCode::BAD_REQUEST)?
            .id()
            .ok_or(StatusCode::BAD_REQUEST)?;
        let mut cache = self
            .state
            .lock()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        if !db
            .session_get(&session.client_id)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .is_some_and(|current| current.token == session.token)
        {
            return Err(StatusCode::UNAUTHORIZED);
        }
        cache.admit(
            key,
            id,
            plain_libs::chrono::Utc::now().timestamp_millis(),
            Instant::now(),
            SESSION_LIMIT,
            TOTAL_LIMIT,
        )
    }
}

impl Cache {
    fn admit(
        &mut self,
        key: SessionKey,
        id: RequestId,
        wall: i64,
        now: Instant,
        session_limit: usize,
        total_limit: usize,
    ) -> Result<(), StatusCode> {
        // A backward wall-clock jump must not revive IDs after monotonic expiry.
        let floor = self.clock_wall.saturating_add(
            now.duration_since(self.clock_instant)
                .as_millis()
                .min(i64::MAX as u128) as i64,
        );
        let wall = wall.max(floor);
        if wall > floor {
            self.clock_wall = wall;
            self.clock_instant = now;
        }
        if !id.is_fresh(wall, WINDOW_MS) {
            return Err(StatusCode::BAD_REQUEST);
        }
        if now.duration_since(self.last_cleanup) >= CLEANUP_INTERVAL {
            self.total = 0;
            self.sessions.retain(|_, seen| {
                seen.retain(|_, inserted| {
                    now.duration_since(*inserted).as_millis() <= RETENTION.as_millis()
                });
                if seen.len().saturating_mul(4) < seen.capacity() {
                    seen.shrink_to_fit();
                }
                self.total += seen.len();
                !seen.is_empty()
            });
            self.last_cleanup = now;
        }
        if self
            .sessions
            .get(&key)
            .is_some_and(|seen| seen.contains_key(&id))
        {
            return Err(StatusCode::BAD_REQUEST);
        }
        if self.total >= total_limit
            || self
                .sessions
                .get(&key)
                .is_some_and(|seen| seen.len() >= session_limit)
        {
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }
        self.sessions.entry(key).or_default().insert(id, now);
        self.total += 1;
        Ok(())
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-server/unit/http/auth/graphql_replay.rs"]
mod tests;
