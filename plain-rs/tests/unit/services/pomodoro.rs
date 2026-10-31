use super::*;
use serde_json::{Value, json};

struct Fixture {
    _directory: tempfile::TempDir,
    service: Arc<Service>,
}

fn fixture() -> Fixture {
    let directory = tempfile::tempdir().unwrap();
    let db = Arc::new(Db::open(&directory.path().join("plain.db")).unwrap());
    let prefs = Arc::new(Prefs::load(&directory.path().join("prefs.json")).unwrap());
    let mut settings = plain_rs::prefs::settings::Settings::default().pomodoro_settings;
    for (key, value) in [
        ("workDurationMin", 1),
        ("shortBreakDurationMin", 2),
        ("longBreakDurationMin", 3),
        ("pomodorosBeforeLongBreak", 2),
    ] {
        settings[key] = json!(value);
    }
    prefs.set("pomodoro_settings", settings).unwrap();
    let (events, _) = broadcast::channel(128);
    Fixture {
        _directory: directory,
        service: Service::new(db, prefs, events),
    }
}

fn noon(day: u32) -> DateTime<Utc> {
    chrono::NaiveDate::from_ymd_opt(2026, 10, day)
        .unwrap()
        .and_hms_opt(12, 0, 0)
        .unwrap()
        .and_local_timezone(chrono::Local)
        .earliest()
        .unwrap()
        .with_timezone(&Utc)
}

#[test]
fn completion_is_atomic_and_shared_phase_rules_match_the_app_baseline() {
    let fixture = fixture();
    let service = fixture.service;
    let now = noon(3);
    let mut events = service.events.subscribe();
    service.command_at("start", Some(1), now).unwrap();
    while events.try_recv().is_ok() {}
    let results = std::thread::scope(|scope| {
        let threads: Vec<_> = (0..16)
            .map(|_| scope.spawn(|| service.tick_at(now + chrono::Duration::seconds(1)).unwrap()))
            .collect();
        threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(
        std::iter::from_fn(|| events.try_recv().ok())
            .filter(|event| {
                let payload: Value = serde_json::from_str(&event.payload).unwrap();
                payload["action"] == "complete" && payload["completedState"] == "WORK"
            })
            .count(),
        1
    );
    let row = service
        .db
        .pomodoro_get_by_date("2026-10-03")
        .unwrap()
        .unwrap();
    assert_eq!((row.completed_count, row.total_work_secs), (1, 60));
    assert!(
        results
            .iter()
            .all(|result| result.state == PomodoroState::ShortBreak && !result.is_running)
    );
    service.command_at("start", Some(1), now).unwrap();
    let rest = service.tick_at(now + chrono::Duration::seconds(1)).unwrap();
    assert_eq!(rest.state, PomodoroState::Work);
    assert_eq!(rest.current_round, 2);
    service.command_at("start", Some(1), now).unwrap();
    let work = service.tick_at(now + chrono::Duration::seconds(1)).unwrap();
    assert_eq!(work.state, PomodoroState::LongBreak);
    assert_eq!(work.time_left_sec, 180);
    service.command_at("stop", None, now).unwrap();
    let row = service
        .db
        .pomodoro_get_by_date("2026-10-03")
        .unwrap()
        .unwrap();
    assert_eq!(
        (
            row.completed_count,
            row.total_work_secs,
            row.total_break_secs
        ),
        (2, 120, 120)
    );
}

#[test]
fn progress_does_not_write_statistics_and_pause_adjust_and_settings_preserve_state() {
    let fixture = fixture();
    let service = fixture.service;
    let now = noon(3);
    for invalid in [0, 86401] {
        assert!(service.command_at("start", Some(invalid), now).is_err());
    }
    service.command_at("start", Some(30), now).unwrap();
    let before = service
        .db
        .with_conn(|c| Ok::<_, anyhow::Error>(c.total_changes()))
        .unwrap();
    for second in 1..10 {
        let progress = service
            .advance_at(now + chrono::Duration::seconds(second), true)
            .unwrap();
        assert_eq!(progress.time_left_sec, 30 - second as i32);
    }
    assert_eq!(
        service
            .db
            .with_conn(|c| Ok::<_, anyhow::Error>(c.total_changes()))
            .unwrap(),
        before
    );
    let paused = service
        .command_at("pause", None, now + chrono::Duration::seconds(10))
        .unwrap();
    assert_eq!(paused.time_left_sec, 20);
    assert!(paused.is_paused && !paused.is_running);
    assert_eq!(
        service
            .tick_at(now + chrono::Duration::hours(1))
            .unwrap()
            .time_left_sec,
        20
    );
    service.command_at("stop", None, now).unwrap();
    service.command_at("adjust", Some(15), now).unwrap();
    assert_eq!(service.tick_at(now).unwrap().time_left_sec, 15);
    let mut settings = serde_json::to_value(service.settings().unwrap()).unwrap();
    settings["workDurationMin"] = json!(3);
    service.prefs.set("pomodoro_settings", settings).unwrap();
    assert_eq!(service.tick_at(now).unwrap().time_left_sec, 180);
    assert!(service.command_at("adjust", Some(-1), now).is_err());
    assert!(service.command_at("adjust", Some(181), now).is_err());
}

#[test]
fn rust_rolls_over_calendar_days_without_client_configuration() {
    let fixture = fixture();
    let service = fixture.service;
    let now = noon(3);
    service.command_at("start", Some(1), now).unwrap();
    service.tick_at(now + chrono::Duration::seconds(1)).unwrap();
    service.command_at("start", Some(1), now).unwrap();
    service.tick_at(now + chrono::Duration::seconds(1)).unwrap();
    assert_eq!(service.tick_at(now).unwrap().current_round, 2);
    let midnight = Service::initial_runtime(&service.settings().unwrap(), noon(4))
        .unwrap()
        .day_start;
    service
        .command_at("start", Some(1), midnight - chrono::Duration::seconds(1))
        .unwrap();
    let next = service.tick_at(midnight).unwrap();
    assert_eq!(next.current_round, 1);
    assert_eq!(next.completed_count, 1);
    assert_eq!(
        service
            .db
            .pomodoro_get_by_date("2026-10-03")
            .unwrap()
            .unwrap()
            .completed_count,
        1
    );
    assert_eq!(
        service
            .db
            .pomodoro_get_by_date("2026-10-04")
            .unwrap()
            .unwrap()
            .completed_count,
        1
    );
}

#[test]
fn failed_completion_keeps_deadline_and_emits_no_success_until_commit() {
    let fixture = fixture();
    let service = fixture.service;
    let mut events = service.events.subscribe();
    let now = noon(3);
    service.command_at("start", Some(1), now).unwrap();
    while events.try_recv().is_ok() {}
    service.db.with_conn(|c| c.execute_batch("CREATE TRIGGER fail_pomodoro BEFORE INSERT ON pomodoro_items BEGIN SELECT RAISE(FAIL, 'test failure'); END")).unwrap();
    assert!(service.tick_at(now + chrono::Duration::seconds(1)).is_err());
    assert!(events.try_recv().is_err());
    assert!(service.runtime.lock().as_ref().unwrap().deadline.is_some());
    service
        .db
        .with_conn(|c| c.execute_batch("DROP TRIGGER fail_pomodoro"))
        .unwrap();
    let done = service.tick_at(now + chrono::Duration::seconds(1)).unwrap();
    assert_eq!(done.completed_count, 1);
    let event: Value = serde_json::from_str(&events.try_recv().unwrap().payload).unwrap();
    assert_eq!(event["action"], "complete");
    assert_eq!(event["completedState"], "WORK");
    assert_eq!(event["today"]["completedCount"], 1);
    assert!(events.try_recv().is_err());
}

#[tokio::test]
async fn lifecycle_completes_without_any_client_tick_or_query_and_stops() {
    let fixture = fixture();
    let service = fixture.service;
    let mut events = service.events.subscribe();
    let (stop, receiver) = watch::channel(false);
    let task = service.follow(receiver);
    service.command("start", Some(30)).unwrap();
    service.command("adjust", Some(0)).unwrap();
    let completed = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let event = events.recv().await.unwrap();
            let payload: Value = serde_json::from_str(&event.payload).unwrap();
            if payload["action"] == "complete" {
                break payload;
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(completed["today"]["completedCount"], 1);
    assert_eq!(completed["today"]["state"], "SHORT_BREAK");
    assert_eq!(completed["today"]["isRunning"], false);
    assert_eq!(completed["completedState"], "WORK");
    stop.send(true).unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), task)
        .await
        .unwrap()
        .unwrap();
    while events.try_recv().is_ok() {}
    assert!(events.try_recv().is_err());
}

#[test]
fn zero_remaining_without_a_running_deadline_does_not_complete_or_record_work() {
    let fixture = fixture();
    let service = fixture.service;
    let now = noon(3);
    let mut events = service.events.subscribe();
    service.command_at("adjust", Some(0), now).unwrap();
    while events.try_recv().is_ok() {}
    for second in [0, 1, 3600] {
        let today = service
            .tick_at(now + chrono::Duration::seconds(second))
            .unwrap();
        assert_eq!(today.state, PomodoroState::Work);
        assert_eq!(today.current_round, 1);
        assert_eq!(today.completed_count, 0);
        assert_eq!(today.time_left_sec, 0);
        assert!(!today.is_running);
    }
    assert!(events.try_recv().is_err());
    assert!(
        service
            .db
            .pomodoro_get_by_date("2026-10-03")
            .unwrap()
            .is_none()
    );
    service.command_at("start", Some(1), now).unwrap();
    while events.try_recv().is_ok() {}
    let today = service.tick_at(now + chrono::Duration::seconds(1)).unwrap();
    assert_eq!(today.state, PomodoroState::ShortBreak);
    assert_eq!(today.completed_count, 1);
    let event: Value = serde_json::from_str(&events.try_recv().unwrap().payload).unwrap();
    assert_eq!(event["action"], "complete");
    assert_eq!(event["completedState"], "WORK");
    service.tick_at(now + chrono::Duration::seconds(2)).unwrap();
    assert!(events.try_recv().is_err());
}

#[tokio::test]
async fn stopped_lifecycle_performs_no_initial_work() {
    let fixture = fixture();
    let service = fixture.service;
    let mut events = service.events.subscribe();
    let (_stop, receiver) = watch::channel(true);
    service.follow(receiver).await.unwrap();
    assert!(service.runtime.lock().is_none());
    assert!(events.try_recv().is_err());
}

#[test]
fn core_restart_restores_statistics_but_does_not_restart_a_timer() {
    let fixture = fixture();
    let service = fixture.service;
    let now = noon(3);
    service.command_at("start", Some(1), now).unwrap();
    service.tick_at(now + chrono::Duration::seconds(1)).unwrap();
    service.command_at("start", Some(30), now).unwrap();
    let next = Service::new(
        service.db.clone(),
        service.prefs.clone(),
        service.events.clone(),
    );
    let today = next.tick_at(now).unwrap();
    assert_eq!(today.completed_count, 1);
    assert_eq!(today.state, PomodoroState::Work);
    assert!(!today.is_running && !today.is_paused);
    assert_eq!(today.time_left_sec, 60);
    let count = next
        .db
        .with_conn(|c| {
            c.query_row(
                "SELECT count(*) FROM sqlite_master WHERE name='pomodoro_runtime'",
                [],
                |r| r.get::<_, i32>(0),
            )
        })
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn idle_lifecycle_initialization_does_not_emit_a_false_progress_change() {
    let fixture = fixture();
    let service = fixture.service;
    let mut events = service.events.subscribe();
    let (stop, receiver) = watch::channel(false);
    let task = service.follow(receiver);
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while service.runtime.lock().is_none() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    stop.send(true).unwrap();
    task.await.unwrap();
    assert!(events.try_recv().is_err());
}
