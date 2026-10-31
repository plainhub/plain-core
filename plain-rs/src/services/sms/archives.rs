use anyhow::{Result, anyhow};
use serde_json::Value;
use std::{
    fs,
    path::Path,
    time::{Duration, UNIX_EPOCH},
};

const RETENTION_MS: i64 = 30 * 24 * 60 * 60 * 1000;

pub fn write(directory: &Path, messages: &[Value], now_ms: i64) -> Result<()> {
    if messages.is_empty() {
        return Ok(());
    }
    fs::create_dir_all(directory)?;
    let name = format!("deleted_{now_ms}_{}", uuid::Uuid::new_v4());
    fs::write(
        directory.join(format!("{name}.json")),
        serde_json::to_vec_pretty(messages)?,
    )?;
    fs::write(directory.join(format!("{name}.csv")), csv(messages)?)?;
    let cutoff =
        UNIX_EPOCH + Duration::from_millis(u64::try_from(now_ms.saturating_sub(RETENTION_MS))?);
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("deleted_")
            && (name.ends_with(".json") || name.ends_with(".csv"))
            && entry.file_type()?.is_file()
            && entry.metadata()?.modified()? < cutoff
        {
            fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

fn csv(messages: &[Value]) -> Result<String> {
    let mut output = String::from("id,body,address,date,thread_id,is_mms");
    for message in messages {
        let text = |name: &str| {
            message[name]
                .as_str()
                .ok_or_else(|| anyhow!("SMS archive field missing: {name}"))
        };
        let date = chrono::DateTime::parse_from_rfc3339(text("date")?)?
            .timestamp_millis()
            .to_string();
        let body = text("body")?.replace('\n', "\\n");
        let is_mms = message["isMms"]
            .as_bool()
            .ok_or_else(|| anyhow!("SMS archive MMS flag missing"))?
            .to_string();
        let fields = [
            text("id")?,
            &body,
            text("address")?,
            &date,
            text("threadId")?,
            &is_mms,
        ];
        output.push('\n');
        output.push_str(
            &fields
                .iter()
                .map(|value| format!("\"{}\"", value.replace('"', "\"\"")))
                .collect::<Vec<_>>()
                .join(","),
        );
    }
    Ok(output)
}

#[cfg(test)]
#[path = "../../../tests/unit/services/sms/archives.rs"]
mod tests;
