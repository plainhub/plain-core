use plain_rs::host::bridge::Host;
use plain_rs::{
    enums::DataType,
    media::actions::{Action, Item, Outcome},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Row {
    id: String,
    path: String,
    trashed: bool,
    mime_type: String,
    volume_root: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Write {
    id: String,
    expected_path: String,
    expected_trash: bool,
    delete: bool,
    set_trash: Option<bool>,
    relative_path: Option<String>,
    display_name: Option<String>,
}
struct Planned {
    before: Row,
    write: Write,
    destination: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Receipt {
    id: String,
    count: i32,
    row: Option<Row>,
}

pub async fn run(
    host: &Host,
    platform: &plain_rs::services::files::platform::FilePlatform,
    kind: DataType,
    action: Action,
    ids: &[String],
    from_trash: bool,
    destination: &str,
) -> Result<Outcome, String> {
    let rows: Vec<Row> = serde_json::from_value(
        host.call(
            "mediaProviderRows",
            json!({"type":kind.kind(),"ids":ids,"includeVolumes":action == Action::Move}),
        )
        .await?,
    )
    .map_err(|error| error.to_string())?;
    let destination = if action == Action::Move {
        platform
            .authorize(destination, true)
            .await
            .map_err(|error| error.to_string())?;
        let path = PathBuf::from(destination);
        Some(
            tokio::task::spawn_blocking(move || {
                std::fs::create_dir_all(&path).map_err(|error| error.to_string())?;
                path.canonicalize().map_err(|error| error.to_string())
            })
            .await
            .map_err(|error| error.to_string())??,
        )
    } else {
        None
    };
    let ids = ids.to_vec();
    let (planned, mut failed) = tokio::task::spawn_blocking(move || {
        plan(kind, action, &ids, from_trash, destination.as_deref(), rows)
    })
    .await
    .map_err(|error| error.to_string())??;
    if planned.is_empty() {
        return Ok(Outcome {
            successful: Vec::new(),
            failed_ids: failed,
        });
    }
    let writes: Vec<_> = planned.iter().map(|item| &item.write).collect();
    let receipts: Vec<Receipt> = serde_json::from_value(
        host.call(
            "mediaProviderWrite",
            json!({"type":kind.kind(),"writes":writes}),
        )
        .await?,
    )
    .map_err(|error| error.to_string())?;
    let successful = tokio::task::spawn_blocking(move || {
        evaluate(action, planned, receipts, &mut failed).map(|successful| Outcome {
            successful,
            failed_ids: failed,
        })
    })
    .await
    .map_err(|error| error.to_string())??;
    Ok(successful)
}

fn plan(
    kind: DataType,
    action: Action,
    ids: &[String],
    from_trash: bool,
    destination: Option<&Path>,
    rows: Vec<Row>,
) -> Result<(Vec<Planned>, Vec<String>), String> {
    let expected: HashSet<_> = ids.iter().collect();
    let mut facts = HashMap::new();
    for row in rows {
        if !expected.contains(&row.id) || facts.insert(row.id.clone(), row).is_some() {
            return Err("Invalid media provider rows".into());
        }
    }
    let mut planned = Vec::new();
    let mut failed = Vec::new();
    let mut reserved = HashSet::new();
    for id in ids {
        let item = (|| -> Result<Planned, String> {
            let before = facts.remove(id).ok_or("Media row not found")?;
            if !Path::new(&before.path).is_absolute()
                || (kind == DataType::Doc
                    && !plain_rs::media::documents::is_document_mime(&before.mime_type))
            {
                return Err("Invalid media provider row".into());
            }
            let mut write = Write {
                id: id.clone(),
                expected_path: before.path.clone(),
                expected_trash: before.trashed,
                delete: action == Action::Delete,
                set_trash: None,
                relative_path: None,
                display_name: None,
            };
            let mut target = String::new();
            match action {
                Action::Delete if before.trashed != from_trash => {
                    return Err("Media trash state changed".into());
                }
                Action::Trash | Action::Restore => {
                    let trashed = action == Action::Trash;
                    if before.trashed == trashed {
                        return Err("Media trash state unchanged".into());
                    }
                    write.set_trash = Some(trashed);
                }
                Action::Move => {
                    let source = Path::new(&before.path);
                    let destination = destination.ok_or("Media destination required")?;
                    if before.trashed
                        || !source.is_file()
                        || source
                            .parent()
                            .ok_or("Media source parent required")?
                            .canonicalize()
                            .map_err(|error| error.to_string())?
                            == destination
                    {
                        return Err("Media source is not movable".into());
                    }
                    let root = Path::new(
                        before
                            .volume_root
                            .as_deref()
                            .ok_or("Media storage volume required")?,
                    )
                    .canonicalize()
                    .map_err(|error| error.to_string())?;
                    let relative = destination
                        .strip_prefix(&root)
                        .map_err(|_| "Media move must stay in its storage volume")?;
                    let path = plain_rs::utils::unique_path::unique_sibling_excluding(
                        &destination.join(source.file_name().ok_or("Media file name required")?),
                        |path| reserved.contains(path),
                    )
                    .map_err(|error| error.to_string())?;
                    reserved.insert(path.clone());
                    write.relative_path = Some(format!(
                        "{}/",
                        relative
                            .to_string_lossy()
                            .replace('\\', "/")
                            .trim_end_matches('/')
                    ));
                    write.display_name = Some(
                        path.file_name()
                            .ok_or("Media target name required")?
                            .to_string_lossy()
                            .into_owned(),
                    );
                    target = path.to_string_lossy().into_owned();
                }
                _ => {}
            }
            Ok(Planned {
                before,
                write,
                destination: target,
            })
        })();
        match item {
            Ok(item) => planned.push(item),
            Err(_) => failed.push(id.clone()),
        }
    }
    Ok((planned, failed))
}

fn evaluate(
    action: Action,
    planned: Vec<Planned>,
    receipts: Vec<Receipt>,
    failed: &mut Vec<String>,
) -> Result<Vec<Item>, String> {
    let expected: HashSet<_> = planned.iter().map(|item| item.before.id.clone()).collect();
    let mut facts = HashMap::new();
    for receipt in receipts {
        if !expected.contains(&receipt.id) || facts.insert(receipt.id.clone(), receipt).is_some() {
            return Err("Invalid media write receipts".into());
        }
    }
    let mut successful = Vec::new();
    for item in planned {
        let changed = facts
            .remove(&item.before.id)
            .filter(|receipt| receipt.count > 0)
            .is_some_and(|receipt| match action {
                Action::Delete => receipt.row.is_none(),
                Action::Trash | Action::Restore => receipt.row.as_ref().is_some_and(|row| {
                    row.id == item.before.id && row.trashed == (action == Action::Trash)
                }),
                Action::Move => {
                    receipt.row.as_ref().is_some_and(|row| {
                        row.id == item.before.id && !row.trashed && row.path == item.destination
                    }) && Path::new(&item.destination).is_file()
                        && !Path::new(&item.before.path).exists()
                }
            });
        if changed {
            successful.push(Item {
                id: item.before.id,
                path: item.before.path,
                destination_path: item.destination,
            });
        } else {
            failed.push(item.before.id);
        }
    }
    Ok(successful)
}

#[cfg(test)]
#[path = "../../../tests/unit/services/files/media_provider.rs"]
mod tests;
