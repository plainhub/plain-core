use crate::server::ContentServer;
use chrono::Utc;
use plain_rs::events::WS_POMODORO_ACTION;
use plain_rs::prefs::Prefs;
use serde_json::{Value, json};
use std::sync::Arc;

struct Fixture {
    _directory: tempfile::TempDir,
    prefs: Arc<Prefs>,
}

fn fixture() -> Fixture {
    let directory = tempfile::tempdir().unwrap();
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
    Fixture {
        _directory: directory,
        prefs,
    }
}

async fn call(server: &ContentServer, token: &str, query: &str) -> Value {
    let text = reqwest::Client::new()
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
    serde_json::from_str(&text).unwrap()
}

#[tokio::test]
async fn server_assembly_runs_the_timer_and_main_controls_share_its_statistics() {
    let fixture = fixture();
    let token = plain_rs::utils::base64::base64_encode(&[12; 32]);
    let server =
        ContentServer::start(fixture._directory.path(), &token, fixture.prefs.clone()).unwrap();
    let state = server.runtime_state();
    let mut events = state.events.subscribe();
    for query in [
        "mutation {startPomodoro(durationSec:0)}",
        "mutation {adjustPomodoro(durationSec:-1) {state}}",
    ] {
        assert!(call(&server, &token, query).await.get("errors").is_some());
    }
    for query in [
        "mutation {startPomodoro(durationSec:30)}",
        "mutation {pausePomodoro}",
    ] {
        assert!(call(&server, &token, query).await.get("errors").is_none());
    }
    assert_eq!(
        call(&server, &token, "{pomodoroToday {isRunning isPaused}} ").await["data"]["pomodoroToday"],
        json!({"isRunning":false,"isPaused":true})
    );
    call(&server, &token, "mutation {startPomodoro(durationSec:30)}").await;
    call(
        &server,
        &token,
        "mutation {adjustPomodoro(durationSec:0) {state}}",
    )
    .await;
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let event = events.recv().await.unwrap();
            if event.event_type == WS_POMODORO_ACTION {
                let payload: Value = serde_json::from_str(&event.payload).unwrap();
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
        .with_timezone(&chrono::Local)
        .date_naive()
        .to_string();
    let row = state.db.pomodoro_get_by_date(&date).unwrap().unwrap();
    assert_eq!(row.completed_count, 1);
    assert_eq!(row.total_work_secs, 60);
    assert_eq!(state.db.pomodoro_list().unwrap().len(), 1);
    call(
        &server,
        &token,
        "mutation {adjustPomodoro(durationSec:0) {state}}",
    )
    .await;
    assert_eq!(
        call(&server, &token, "mutation {stopPomodoro}").await["data"]["stopPomodoro"],
        true
    );
    server.shutdown().await;
}

#[tokio::test]
async fn main_schema_rejects_retired_client_scheduler_controls() {
    let fixture = fixture();
    let token = plain_rs::utils::base64::base64_encode(&[12; 32]);
    let server =
        ContentServer::start(fixture._directory.path(), &token, fixture.prefs.clone()).unwrap();
    for query in [
        "mutation { tickPomodoro {completedState} }",
        "mutation { skipPomodoro {completedState} }",
        "{pomodoroTotalCompleted}",
        "mutation { configurePomodoroDay(date:\"2026-10-03\", dayStart:\"2026-10-03T00:00:00Z\") {date} }",
    ] {
        let result: Value = reqwest::Client::new()
            .post(format!("http://127.0.0.1:{}/graphql", server.port))
            .bearer_auth(&token)
            .header("content-type", "application/json")
            .body(json!({"query":query}).to_string())
            .send()
            .await
            .unwrap()
            .text()
            .await
            .map(|text| serde_json::from_str(&text).unwrap())
            .unwrap();
        assert!(result.get("errors").is_some(), "{result}");
    }
    server.shutdown().await;
}
