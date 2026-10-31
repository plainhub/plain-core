use super::SettingsPatch;
use anyhow::{Result, ensure};
use serde_json::Value;

fn strings(value: &Value) -> Result<Vec<String>> {
    Ok(serde_json::from_value(value.clone())?)
}
fn fields(value: &Value, names: &[&str], predicate: fn(&Value) -> bool) -> Result<()> {
    ensure!(value.is_object(), "Expected a settings object");
    for name in names {
        ensure!(
            value.get(*name).is_some_and(predicate),
            "Invalid settings field: {name}"
        );
    }
    Ok(())
}
fn positive(value: &Value) -> bool {
    value
        .as_i64()
        .is_some_and(|n| n > 0 && n <= i32::MAX as i64 / 60)
}

pub(super) fn validate(patch: &SettingsPatch) -> Result<()> {
    for port in [patch.http_port, patch.https_port].into_iter().flatten() {
        ensure!((0..=65535).contains(&port), "Invalid HTTP port");
    }
    for cells in [
        patch.image_grid_cells_per_row,
        patch.video_grid_cells_per_row,
    ]
    .into_iter()
    .flatten()
    {
        ensure!(cells > 0, "Invalid grid size");
    }
    if let Some(speed) = patch.audio_playback_speed {
        ensure!(speed.is_finite() && speed > 0.0, "Invalid playback speed");
    }
    for value in [patch.feed_auto_refresh_interval, patch.editor_font_size]
        .into_iter()
        .flatten()
    {
        ensure!(value > 0, "Invalid settings value");
    }
    if let Some(value) = &patch.screen_mirror_quality {
        fields(value, &["resolution"], positive)?;
        ensure!(
            matches!(value["mode"].as_str(), Some("HD" | "SMOOTH")),
            "Invalid screen mirror mode"
        );
    }
    if let Some(value) = &patch.notification_filter {
        ensure!(
            matches!(value["mode"].as_str(), Some("allowlist" | "blacklist")),
            "Invalid notification mode"
        );
        strings(&value["apps"])?;
    }
    if let Some(value) = &patch.pomodoro_settings {
        fields(
            value,
            &[
                "workDurationMin",
                "shortBreakDurationMin",
                "longBreakDurationMin",
                "pomodorosBeforeLongBreak",
            ],
            positive,
        )?;
        fields(
            value,
            &["showNotification", "playSoundOnComplete"],
            Value::is_boolean,
        )?;
        fields(value, &["soundPath", "originalSoundName"], Value::is_string)?;
    }
    Ok(())
}
