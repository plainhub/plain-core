use plain_libs::protocols::http_client;
use crate::server::ContentServer;
use plain_libs::chrono::Utc;
use plain_rs::features::prefs::Prefs;
use plain_rs::infrastructure::events::WS_POMODORO_ACTION;
use plain_libs::serde_json::{Value, json};
use std::sync::Arc;

struct Fixture {
    _directory: plain_libs::temporary::TempDir,
    prefs: Arc<Prefs>,
}

fn fixture() -> Fixture {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let prefs = Arc::new(Prefs::load(&directory.path().join("prefs.json")).unwrap());
    let mut settings = plain_rs::features::prefs::settings::Settings::default().pomodoro_settings;
    for (key, value) in [
        ("workDurationMin", 1),
        ("shortBreakDurationMin", 2),
        ("longBreakDurationMin", 3),
        ("pomodorosBeforeLongBreak", 2),
    ] {
        settings[key] = json!(value);
    }
    prefs.set("pomodoro_settings", settings).unwrap();
    Fixture {
        _directory: directory,
        prefs,
    }
}

async fn call(server: &ContentServer, token: &str, query: &str) -> Value {
    let text = http_client::builder().build().unwrap()
        .post(format!("http://127.0.0.1:{}/graphql", server.port))
        .bearer_auth(token)
        .header("content-type", "application/json")
        .body(json!({"query":query}).to_string())
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    plain_libs::serde_json::from_str(&text).unwrap()
}

#[tokio::test]
async fn server_assembly_runs_the_timer_and_main_controls_share_its_statistics() {
    let fixture = fixture();
    let token = plain_libs::utils::base64::base64_encode(&[12; 32]);
    let server =
        ContentServer::start(fixture._directory.path(), &token, fixture.prefs.clone()).unwrap();
    let state = server.runtime_state();
    let mut events = state.events.subscribe();
    for query in [
        "mutation {startPomodoro(durationSec:0) {timeLeftSec totalTimeSec isRunning isPaused state}}",
        "mutation {setPomodoroRemainingTime(remainingSec:-1) {state}}",
    ] {
        assert!(call(&server, &token, query).await.get("errors").is_some());
    }
    let started = call(&server, &token, "mutation {startPomodoro(durationSec:30) {isRunning isPaused totalTimeSec}}").await;
    assert!(started.get("errors").is_none(), "{started}");
    assert_eq!(started["data"]["startPomodoro"], json!({"isRunning":true,"isPaused":false,"totalTimeSec":60}));
    let paused = call(&server, &token, "mutation {pausePomodoro {isRunning isPaused}}").await;
    assert!(paused.get("errors").is_none(), "{paused}");
    assert_eq!(paused["data"]["pausePomodoro"], json!({"isRunning":false,"isPaused":true}));
    let changed = call(&server, &token, "mutation {setPomodoroRemainingTime(remainingSec:17) {timeLeftSec totalTimeSec isRunning isPaused}}").await;
    assert!(changed.get("errors").is_none(), "{changed}");
    assert_eq!(changed["data"]["setPomodoroRemainingTime"], json!({"timeLeftSec":17,"totalTimeSec":60,"isRunning":false,"isPaused":true}));
    assert_eq!(
        call(&server, &token, "{pomodoroToday {isRunning isPaused}} ").await["data"]["pomodoroToday"],
        json!({"isRunning":false,"isPaused":true})
    );
    call(&server, &token, "mutation {startPomodoro(durationSec:30) {timeLeftSec totalTimeSec isRunning isPaused state}}").await;
    call(
        &server,
        &token,
        "mutation {setPomodoroRemainingTime(remainingSec:0) {state}}",
    )
    .await;
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let event = events.recv().await.unwrap();
            if event.event_type == WS_POMODORO_ACTION {
                let payload: Value = plain_libs::serde_json::from_str(&event.payload).unwrap();
                if payload["action"] == "complete" {
                    break;
                }
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(
        call(
            &server,
            &token,
            "{pomodoroToday {completedCount state isRunning}} "
        )
        .await["data"],
        json!({"pomodoroToday":{"completedCount":1,"state":"SHORT_BREAK","isRunning":false}})
    );
    let date = Utc::now()
        .with_timezone(&plain_libs::local_time::Local)
        .date_naive()
        .to_string();
    let row = state.db.pomodoro_get_by_date(&date).unwrap().unwrap();
    assert_eq!(row.completed_count, 1);
    assert_eq!(row.total_work_secs, 60);
    assert_eq!(state.db.pomodoro_list().unwrap().len(), 1);
    call(
        &server,
        &token,
        "mutation {setPomodoroRemainingTime(remainingSec:0) {state}}",
    )
    .await;
    assert_eq!(
        call(&server, &token, "mutation {stopPomodoro {timeLeftSec totalTimeSec isRunning isPaused state}}").await["data"]["stopPomodoro"],
        json!({"timeLeftSec":60,"totalTimeSec":60,"isRunning":false,"isPaused":false,"state":"WORK"})
    );
    server.shutdown().await;
}

#[tokio::test]
async fn main_schema_rejects_retired_client_scheduler_controls() {
    let fixture = fixture();
    let token = plain_libs::utils::base64::base64_encode(&[12; 32]);
    let server =
        ContentServer::start(fixture._directory.path(), &token, fixture.prefs.clone()).unwrap();
    for query in [
        "mutation { adjustPomodoro(durationSec:10) {state} }",
        "mutation { setPomodoroRemainingTime(durationSec:10) {state} }",
        "mutation { tickPomodoro {completedState} }",
        "mutation { skipPomodoro {completedState} }",
        "{pomodoroTotalCompleted}",
        "mutation { configurePomodoroDay(date:\"2026-10-03\", dayStart:\"2026-10-03T00:00:00Z\") {date} }",
    ] {
        let result: Value = http_client::builder().build().unwrap()
            .post(format!("http://127.0.0.1:{}/graphql", server.port))
            .bearer_auth(&token)
            .header("content-type", "application/json")
            .body(json!({"query":query}).to_string())
            .send()
            .await
            .unwrap()
            .text()
            .await
            .map(|text| plain_libs::serde_json::from_str(&text).unwrap())
            .unwrap();
        assert!(result.get("errors").is_some(), "{result}");
    }
    server.shutdown().await;
}
