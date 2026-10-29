use super::{manifest, splits};
use crate::{assembly::services::Services, host::bridge::Host};
use anyhow::{Result, anyhow, ensure};
use serde_json::{Value, json};
use std::sync::Arc;
use std::{
    fs::File,
    io::{Read, Seek},
    path::{Path, PathBuf},
};

struct Entry {
    name: String,
    path: PathBuf,
    metadata: Option<manifest::Metadata>,
}
struct Prepared {
    package_name: String,
    files: Vec<PathBuf>,
    session: bool,
    _staging: Option<tempfile::TempDir>,
}
fn temporary(cache: &Path) -> Result<tempfile::TempDir> {
    let directory = cache.join("packages");
    std::fs::create_dir_all(&directory)?;
    Ok(tempfile::Builder::new()
        .prefix("install-")
        .tempdir_in(directory)?)
}
fn prepare(
    mut reader: impl Read + Seek,
    extension: &str,
    cache: &Path,
    source: Option<PathBuf>,
    profile: Option<splits::Profile>,
) -> Result<Prepared> {
    if extension == "apk" {
        let metadata = manifest::read(&mut reader)?;
        if let Some(path) = source {
            return Ok(Prepared {
                package_name: metadata.package_name,
                files: vec![path],
                session: false,
                _staging: None,
            });
        }
        let staging = temporary(cache)?;
        let path = staging.path().join("base.apk");
        reader.seek(std::io::SeekFrom::Start(0))?;
        std::io::copy(&mut reader, &mut File::create(&path)?)?;
        return Ok(Prepared {
            package_name: metadata.package_name,
            files: vec![path],
            session: true,
            _staging: Some(staging),
        });
    }
    ensure!(
        matches!(extension, "apkm" | "apks" | "xapk"),
        "Unsupported package archive format"
    );
    let profile = profile.ok_or_else(|| anyhow!("Package installation profile required"))?;
    let mut archive = zip::ZipArchive::new(reader)?;
    let staging = temporary(cache)?;
    let mut entries = Vec::new();
    let mut names = std::collections::HashSet::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let name = entry.name()?.into_owned();
        if entry.is_dir() || !name.to_ascii_lowercase().ends_with(".apk") {
            continue;
        }
        ensure!(names.insert(name.clone()), "Duplicate APK archive entry");
        ensure!(
            entry.enclosed_name().is_some()
                && !name.contains('\\')
                && entry
                    .unix_mode()
                    .is_none_or(|mode| mode & 0o170000 != 0o120000),
            "Unsafe APK archive entry"
        );
        let path = staging.path().join(format!("{index}.apk"));
        let mut output = File::create(&path)?;
        let expected = entry.size();
        let written = std::io::copy(&mut entry, &mut output)?;
        ensure!(written == expected, "APK entry length changed");
        let metadata = manifest::read(File::open(&path)?).ok();
        entries.push(Entry {
            name,
            path,
            metadata,
        });
    }
    ensure!(!entries.is_empty(), "No APK found inside package archive");
    let mains: Vec<_> = entries
        .iter()
        .filter(|entry| !splits::sidecar(&entry.name))
        .collect();
    let base = mains
        .iter()
        .copied()
        .filter(|entry| {
            entry
                .metadata
                .as_ref()
                .is_some_and(|metadata| metadata.split.is_none())
        })
        .min_by_key(|entry| {
            let name = entry.name.rsplit('/').next().unwrap_or(&entry.name);
            if name == "base.apk" {
                0
            } else if name.ends_with("-master.apk") {
                1
            } else {
                2
            }
        });
    let (metadata, files) = if let Some(base) = base {
        let metadata = base.metadata.as_ref().unwrap();
        let names: Vec<_> = mains
            .iter()
            .filter(|entry| !std::ptr::eq(**entry, base))
            .map(|entry| entry.name.clone())
            .collect();
        let selected = splits::select(&names, &profile);
        let mut files = vec![base.path.clone()];
        for entry in &mains {
            if std::ptr::eq(*entry, base) || !selected.contains(&entry.name.as_str()) {
                continue;
            }
            let split = entry
                .metadata
                .as_ref()
                .ok_or_else(|| anyhow!("Invalid selected split manifest"))?;
            ensure!(
                split.package_name == metadata.package_name,
                "Split APK package mismatch"
            );
            files.push(entry.path.clone());
        }
        (metadata, files)
    } else {
        let mut standalones: Vec<_> = entries
            .iter()
            .filter(|entry| entry.metadata.is_some())
            .collect();
        standalones.sort_by(|first, second| first.name.cmp(&second.name));
        let standalone = standalones
            .into_iter()
            .find(|entry| {
                profile.abi_order.iter().any(|abi| {
                    entry
                        .name
                        .to_ascii_lowercase()
                        .contains(&abi.to_ascii_lowercase())
                })
            })
            .or_else(|| entries.iter().find(|entry| entry.metadata.is_some()))
            .ok_or_else(|| anyhow!("No installable APK found inside package archive"))?;
        (
            standalone.metadata.as_ref().unwrap(),
            vec![standalone.path.clone()],
        )
    };
    Ok(Prepared {
        package_name: metadata.package_name.clone(),
        files,
        session: true,
        _staging: Some(staging),
    })
}
pub(crate) async fn execute(services: &Services, host: Arc<Host>, path: &str) -> Result<Value> {
    let path = services.file_platform.resolve_reference(path)?;
    services.file_platform.authorize(&path, false).await?;
    let extension = Path::new(&path)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    ensure!(
        matches!(extension.as_str(), "apk" | "apkm" | "apks" | "xapk"),
        "Unsupported file format. Supported: .apk, .apkm, .apks, .xapk"
    );
    let profile = if extension != "apk" {
        let facts = host
            .call("systemPackageInstallProfile", json!({}))
            .await
            .map_err(anyhow::Error::msg)?;
        let mut profile: splits::Profile = serde_json::from_value(facts)?;
        profile.abi_order = host.startup().supported_abis.clone();
        ensure!(
            profile.density_dpi > 0 && !profile.abi_order.is_empty(),
            "Invalid package install profile"
        );
        Some(profile)
    } else {
        None
    };
    let source = plain_rs::utils::file_uri::local_path(&path).map_err(anyhow::Error::msg)?;
    let local = Path::new(&source)
        .is_absolute()
        .then(|| PathBuf::from(source));
    let cache = services.cache.root().to_owned();
    let prepared = crate::services::files::input::read_source(
        &services.resources,
        services.cache.root(),
        &path,
        move |reader| {
            prepare(reader, &extension, &cache, local, profile).map_err(|error| error.to_string())
        },
    )
    .await
    .map_err(anyhow::Error::msg)?;
    let operation = tokio::spawn(async move {
        let result = host.call_wait("systemInstallPackage", json!({
            "paths": &prepared.files, "packageName": &prepared.package_name, "session": prepared.session,
        })).await;
        drop(prepared);
        result
    });
    operation.await?.map_err(anyhow::Error::msg)
}

#[cfg(test)]
#[path = "../../../tests/unit/services/packages/install.rs"]
mod tests;
