use anyhow::{Result, anyhow, bail};
use chrono::TimeZone;
use plain_rs::files::filesystem::record::FileRecord;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::{Component, Path},
};

pub(crate) fn list(reference: &str) -> Result<Vec<FileRecord>> {
    let (archive, internal) = reference
        .split_once("!zip!/")
        .ok_or_else(|| anyhow!("archive entry reference required"))?;
    let internal = internal.trim_matches('/');
    if !Path::new(archive).is_absolute()
        || Path::new(internal)
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        bail!("invalid archive directory");
    }
    let prefix = if internal.is_empty() {
        String::new()
    } else {
        format!("{internal}/")
    };
    let mut archive_file = zip::ZipArchive::new(std::fs::File::open(archive)?)?;
    let mut entries = BTreeMap::new();
    let mut children: HashMap<String, HashSet<String>> = HashMap::new();
    for index in 0..archive_file.len() {
        let entry = archive_file.by_index(index)?;
        if entry.enclosed_name().is_none() {
            continue;
        }
        let name = entry.name()?;
        let Some(relative) = name
            .trim_start_matches('/')
            .strip_prefix(&prefix)
            .filter(|name| !name.is_empty())
        else {
            continue;
        };
        let (name, tail) = relative
            .split_once('/')
            .map_or((relative, None), |(name, rest)| (name, Some(rest)));
        if name.is_empty() || name == "." || name == ".." {
            continue;
        }
        let directory = tail.is_some() || entry.is_dir();
        if let Some(child) = tail
            .filter(|rest| !rest.is_empty())
            .and_then(|rest| rest.split('/').next())
            .filter(|name| !name.is_empty())
        {
            children
                .entry(name.into())
                .or_default()
                .insert(child.into());
        }
        if entries.contains_key(name) {
            continue;
        }
        let modified = if tail.is_some_and(|rest| !rest.is_empty()) {
            0
        } else {
            entry
                .last_modified()
                .and_then(|date| {
                    chrono::NaiveDate::from_ymd_opt(
                        date.year().into(),
                        date.month().into(),
                        date.day().into(),
                    )
                    .and_then(|day| {
                        day.and_hms_opt(
                            date.hour().into(),
                            date.minute().into(),
                            date.second().into(),
                        )
                    })
                })
                .and_then(|time| chrono::Local.from_local_datetime(&time).earliest())
                .map(|time| time.timestamp_millis().max(0))
                .unwrap_or(0)
        };
        entries.insert(
            name.to_owned(),
            FileRecord {
                name: name.into(),
                path: format!(
                    "{archive}!zip!/{prefix}{name}{}",
                    if directory { "/" } else { "" }
                ),
                permission: String::new(),
                created_at: None,
                updated_at: modified,
                size: if directory {
                    0
                } else {
                    i64::try_from(entry.size())?
                },
                is_dir: directory,
                child_count: 0,
                media_id: String::new(),
            },
        );
    }
    Ok(entries
        .into_values()
        .map(|mut row| {
            row.child_count = children
                .get(&row.name)
                .map_or(0, |items| items.len().min(i32::MAX as usize) as i32);
            row
        })
        .collect())
}
