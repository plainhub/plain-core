use async_graphql::{Enum, SimpleObject};
use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use plain_rs::db::Db;
use plain_rs::events::WS_POMODORO_ACTION;
use plain_rs::events::WsEvent;
use plain_rs::prefs::Prefs;
use plain_rs::types::Instant;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{broadcast, watch};

#[derive(Clone, Deserialize, Serialize, SimpleObject)]
#[serde(rename_all = "camelCase")]
pub struct PomodoroSettings {
    pub work_duration_min: i32,
    pub short_break_duration_min: i32,
    pub long_break_duration_min: i32,
    pub pomodoros_before_long_break: i32,
    pub show_notification: bool,
    pub play_sound_on_complete: bool,
    pub sound_path: String,
    pub original_sound_name: String,
}
impl Default for PomodoroSettings {
    fn default() -> Self {
        serde_json::from_value(plain_rs::prefs::settings::Settings::default().pomodoro_settings)
            .expect("Default pomodoro settings")
    }
}
#[derive(Enum, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Debug)]
#[graphql(rename_items = "SCREAMING_SNAKE_CASE")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PomodoroState {
    Work,
    ShortBreak,
    LongBreak,
}
#[derive(Clone, Serialize, Deserialize, SimpleObject)]
#[serde(rename_all = "camelCase")]
pub struct PomodoroToday {
    pub date: Instant,
    pub completed_count: i32,
    pub current_round: i32,
    pub time_left_sec: i32,
    pub total_time_sec: i32,
    pub is_running: bool,
    pub is_paused: bool,
    pub state: PomodoroState,
}
#[derive(Clone)]
struct Runtime {
    calendar_date: String,
    day_start: DateTime<Utc>,
    round: i32,
    remaining: i32,
    total: i32,
    settings_duration: i32,
    deadline: Option<DateTime<Utc>>,
    paused: bool,
    state: PomodoroState,
}
impl Runtime {
    fn remaining(&self, now: DateTime<Utc>) -> i32 {
        self.deadline
            .map(|d| ((d - now).num_milliseconds().max(0) + 999) / 1000)
            .map(|n| n.min(i32::MAX as i64) as i32)
            .unwrap_or(self.remaining)
    }
}
pub struct Service {
    db: Arc<Db>,
    prefs: Arc<Prefs>,
    events: broadcast::Sender<WsEvent>,
    runtime: Mutex<Option<Runtime>>,
    changed: watch::Sender<u64>,
}
impl Service {
    pub fn new(db: Arc<Db>, prefs: Arc<Prefs>, events: broadcast::Sender<WsEvent>) -> Arc<Self> {
        Arc::new(Self {
            db,
            prefs,
            events,
            runtime: Mutex::new(None),
            changed: watch::channel(0).0,
        })
    }
    pub fn settings(&self) -> anyhow::Result<PomodoroSettings> {
        let settings: PomodoroSettings = serde_json::from_value(
            plain_rs::prefs::settings::snapshot(&self.prefs)?.pomodoro_settings,
        )?;
        anyhow::ensure!(
            (1..=1440).contains(&settings.work_duration_min)
                && (1..=1440).contains(&settings.short_break_duration_min)
                && (1..=1440).contains(&settings.long_break_duration_min)
                && (1..=100).contains(&settings.pomodoros_before_long_break),
            "invalid pomodoro settings"
        );
        Ok(settings)
    }
    fn duration(settings: &PomodoroSettings, state: PomodoroState) -> i32 {
        (match state {
            PomodoroState::Work => settings.work_duration_min,
            PomodoroState::ShortBreak => settings.short_break_duration_min,
            PomodoroState::LongBreak => settings.long_break_duration_min,
        }) * 60
    }
    fn initial_runtime(settings: &PomodoroSettings, now: DateTime<Utc>) -> anyhow::Result<Runtime> {
        let today = now.with_timezone(&chrono::Local).date_naive();
        let day_start = today
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_local_timezone(chrono::Local)
            .earliest()
            .ok_or_else(|| anyhow::anyhow!("invalid local day"))?
            .with_timezone(&Utc);
        Ok(Runtime {
            calendar_date: today.to_string(),
            day_start,
            round: 1,
            remaining: settings.work_duration_min * 60,
            total: settings.work_duration_min * 60,
            settings_duration: settings.work_duration_min * 60,
            deadline: None,
            paused: false,
            state: PomodoroState::Work,
        })
    }
    fn runtime(
        &self,
        settings: &PomodoroSettings,
        now: DateTime<Utc>,
    ) -> anyhow::Result<parking_lot::MutexGuard<'_, Option<Runtime>>> {
        let mut runtime = self.runtime.lock();
        let local = Self::initial_runtime(settings, now)?;
        if let Some(r) = runtime.as_mut() {
            if r.calendar_date != local.calendar_date {
                r.round = 1;
            }
            r.calendar_date = local.calendar_date;
            r.day_start = local.day_start;
            let duration = Self::duration(settings, r.state);
            if r.deadline.is_none() && !r.paused && r.settings_duration != duration {
                r.remaining = duration;
                r.total = duration;
                r.settings_duration = duration;
            }
        } else {
            *runtime = Some(local);
        }
        Ok(runtime)
    }
    fn snapshot(
        conn: &rusqlite::Connection,
        runtime: &Runtime,
        now: DateTime<Utc>,
    ) -> anyhow::Result<PomodoroToday> {
        let count: Option<i32> = conn
            .query_row(
                "SELECT completed_count FROM pomodoro_items WHERE date=?1",
                [&runtime.calendar_date],
                |r| r.get(0),
            )
            .optional()?;
        Ok(PomodoroToday {
            date: Instant(runtime.day_start),
            completed_count: count.unwrap_or(0),
            current_round: runtime.round,
            time_left_sec: runtime.remaining(now),
            total_time_sec: runtime.total,
            is_running: runtime.deadline.is_some(),
            is_paused: runtime.paused,
            state: runtime.state,
        })
    }
    pub fn today(&self) -> anyhow::Result<PomodoroToday> {
        self.tick_at(Utc::now())
    }
    pub fn command(&self, action: &str, duration: Option<i32>) -> anyhow::Result<PomodoroToday> {
        let now = Utc::now();
        self.tick_at(now)?;
        self.command_at(action, duration, now)
    }
    fn command_at(
        &self,
        action: &str,
        duration: Option<i32>,
        now: DateTime<Utc>,
    ) -> anyhow::Result<PomodoroToday> {
        let settings = self.settings()?;
        let mut runtime = self.runtime(&settings, now)?;
        let mut r = runtime.as_ref().unwrap().clone();
        let today = self.db.with_conn(|c| {
            match action {
                "start" => {
                    let seconds = duration.unwrap_or(r.remaining(now));
                    anyhow::ensure!((1..=86400).contains(&seconds), "invalid duration");
                    r.remaining = seconds;
                    r.deadline = Some(now + chrono::Duration::seconds(seconds as i64));
                    r.paused = false;
                }
                "pause" => {
                    r.remaining = r.remaining(now);
                    r.deadline = None;
                    r.paused = true;
                }
                "stop" => {
                    r.state = PomodoroState::Work;
                    r.remaining = Self::duration(&settings, r.state);
                    r.total = r.remaining;
                    r.deadline = None;
                    r.paused = false;
                }
                "adjust" => {
                    let seconds = duration.ok_or_else(|| anyhow::anyhow!("duration required"))?;
                    anyhow::ensure!((0..=r.total).contains(&seconds), "invalid duration");
                    r.remaining = seconds;
                    if r.deadline.is_some() {
                        r.deadline = Some(now + chrono::Duration::seconds(seconds as i64));
                    }
                }
                _ => anyhow::bail!("unknown pomodoro action"),
            }
            Self::snapshot(c, &r, now)
        })?;
        *runtime = Some(r);
        self.publish(action, &today, None);
        self.changed
            .send_modify(|revision| *revision = revision.wrapping_add(1));
        Ok(today)
    }
    fn tick_at(&self, now: DateTime<Utc>) -> anyhow::Result<PomodoroToday> {
        self.advance_at(now, false)
    }
    fn advance_at(&self, now: DateTime<Utc>, progress: bool) -> anyhow::Result<PomodoroToday> {
        let settings = self.settings()?;
        let mut runtime = self.runtime(&settings, now)?;
        let mut r = runtime.as_ref().unwrap().clone();
        if !(r.deadline.is_some() && r.remaining(now) == 0) {
            let today = self.db.with_conn(|c| Self::snapshot(c, &r, now))?;
            *runtime = Some(r);
            if progress {
                self.publish("progress", &today, None);
            }
            return Ok(today);
        }
        let result = self.db.with_conn(|conn| {
            let tx = conn.unchecked_transaction()?;
            let completed = r.state;
            let count: Option<i32> = tx.query_row(
                "SELECT completed_count FROM pomodoro_items WHERE date=?1",
                [&r.calendar_date], |row| row.get(0),
            ).optional()?;
            let mut count = count.unwrap_or(0);
            let work = if completed == PomodoroState::Work { r.total } else { 0 };
            let rest = if completed != PomodoroState::Work { r.total } else { 0 };
            let increment = i32::from(completed == PomodoroState::Work);
            let existing: Option<String> = tx.query_row(
                "SELECT id FROM pomodoro_items WHERE date=?1",
                [&r.calendar_date], |row| row.get(0),
            ).optional()?;
            let timestamp = now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
            if let Some(id) = existing {
                tx.execute(
                    "UPDATE pomodoro_items SET completed_count=completed_count+?1,total_work_secs=total_work_secs+?2,total_break_secs=total_break_secs+?3,updated_at=?4 WHERE id=?5",
                    params![increment, work, rest, timestamp, id],
                )?;
            } else {
                tx.execute(
                    "INSERT INTO pomodoro_items(id,date,completed_count,total_work_secs,total_break_secs,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?6)",
                    params![uuid::Uuid::new_v4().to_string(), r.calendar_date, increment, work, rest, timestamp],
                )?;
            }
            count += increment;
            r.state = match completed {
                PomodoroState::Work if count > 0 && count % settings.pomodoros_before_long_break == 0 => PomodoroState::LongBreak,
                PomodoroState::Work => PomodoroState::ShortBreak,
                PomodoroState::ShortBreak | PomodoroState::LongBreak => {
                    r.round += 1;
                    PomodoroState::Work
                }
            };
            r.deadline = None;
            r.paused = false;
            r.remaining = Self::duration(&settings, r.state);
            r.total = r.remaining;
            r.settings_duration = r.total;
            let today = Self::snapshot(&tx, &r, now)?;
            tx.commit()?;
            Ok::<_, anyhow::Error>((today, completed))
        })?;
        *runtime = Some(r);
        let (today, completed) = result;
        self.publish("complete", &today, Some(completed));
        Ok(today)
    }

    pub fn follow(
        self: &Arc<Self>,
        mut stop: watch::Receiver<bool>,
    ) -> tokio::task::JoinHandle<()> {
        let service = self.clone();
        let mut changed = self.changed.subscribe();
        let mut events = self.events.subscribe();
        tokio::spawn(async move {
            let mut initialized = false;
            loop {
                if *stop.borrow() {
                    break;
                }
                let current = service.clone();
                let result = tokio::task::spawn_blocking(move || {
                    current.advance_at(Utc::now(), initialized)
                })
                .await;
                initialized = true;
                let delay = match result {
                    Ok(Ok(today)) if today.is_running => std::time::Duration::from_secs(1),
                    Ok(Ok(_)) => Self::until_next_day(Utc::now()),
                    Ok(Err(error)) => {
                        log::warn!("Pomodoro lifecycle: {error}");
                        std::time::Duration::from_secs(5)
                    }
                    Err(error) => {
                        log::warn!("Pomodoro lifecycle task: {error}");
                        break;
                    }
                };
                tokio::select! {
                    biased;
                    _ = stop.changed() => break,
                    _ = changed.changed() => {},
                    _ = tokio::time::sleep(delay) => {},
                    _ = async {
                        loop {
                            match events.recv().await {
                                Ok(event) if matches!(event.event_type, "PREFS_UPDATED" | "POMODORO_SETTINGS_UPDATE") => break,
                                Ok(_) => {},
                                Err(broadcast::error::RecvError::Lagged(_)) => break,
                                Err(broadcast::error::RecvError::Closed) => break,
                            }
                        }
                    } => {},
                }
            }
        })
    }
    fn until_next_day(now: DateTime<Utc>) -> std::time::Duration {
        let local = now.with_timezone(&chrono::Local);
        local
            .date_naive()
            .succ_opt()
            .and_then(|day| day.and_hms_opt(0, 0, 0))
            .and_then(|date| date.and_local_timezone(chrono::Local).earliest())
            .and_then(|date| (date.with_timezone(&Utc) - now).to_std().ok())
            .filter(|delay| !delay.is_zero())
            .unwrap_or(std::time::Duration::from_secs(60))
    }
    fn publish(&self, action: &str, today: &PomodoroToday, completed_state: Option<PomodoroState>) {
        let payload = serde_json::json!({"action":action,"timeLeftSec":today.time_left_sec,"totalTimeSec":today.total_time_sec,"completedCount":today.completed_count,"round":today.current_round,"state":today.state,"completedState":completed_state,"today":today});
        let _ = self
            .events
            .send(WsEvent::broadcast(WS_POMODORO_ACTION, payload.to_string()));
    }
}

#[cfg(test)]
#[path = "../../tests/unit/services/pomodoro.rs"]
mod tests;
