use super::{assets::Assets, chat, short_id};
use crate::features::prefs::{Prefs, pref_keys};
use crate::infrastructure::{data_paths::DataPaths, db::Db};
use plain_libs::anyhow::{Context, Result, bail, ensure};
use plain_libs::rusqlite::{Connection, OpenFlags};
use plain_libs::serde_json::{self, Value};
use plain_libs::temporary::NamedTempFile;
use plain_libs::utils::base64::{base64_decode_checked, base64_encode};
use serde::Deserialize;
use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

const RECEIPT: &str = "desktop_chat_upgrade_4_0_0";
const ROWS_IMPORTED: &str = "rows_imported";
const COMPLETE: &str = "complete";

#[derive(Deserialize)]
struct Identity {
    client_id: String,
    signature_key_pair: Option<String>,
}

fn read(path: &Path) -> Result<Option<Vec<u8>>> {
    match path.symlink_metadata() {
        Ok(meta) => {
            ensure!(meta.is_file(), "Invalid legacy desktop preferences file");
            Ok(Some(fs::read(path)?))
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn signing_seed(keypair: &str) -> Result<String> {
    let bytes = base64_decode_checked(keypair)?;
    let (seed, public) = bytes
        .split_at_checked(32)
        .context("Invalid desktop signing key")?;
    let seed: [u8; 32] = seed.try_into()?;
    ensure!(
        plain_libs::crypto::ed25519_keypair_from_seed(&seed)[32..] == *public,
        "Invalid desktop signing keypair"
    );
    Ok(base64_encode(&seed))
}

pub(crate) fn source(root: &Path, destination: &Path) -> Result<Option<PathBuf>> {
    let source_path = DataPaths::new(root).legacy_desktop_database();
    match source_path.symlink_metadata() {
        Ok(meta) => ensure!(meta.is_file(), "Invalid legacy desktop database"),
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    }
    if destination.try_exists()? {
        ensure!(
            !plain_libs::utils::file_identity::same(&source_path, destination)?,
            "Legacy desktop database overlaps the current database"
        );
    }
    Ok(Some(source_path))
}

pub(crate) fn migrate(root: &Path, source_path: &Path, db: &Db, prefs: &Prefs) -> Result<()> {
    let receipt = db.key_values().get(RECEIPT)?;
    match receipt.as_deref() {
        Some(COMPLETE) => return Ok(()),
        Some(ROWS_IMPORTED) | None => {}
        _ => bail!("Invalid Desktop migration receipt"),
    }

    let paths = DataPaths::new(root);
    let backup = paths.legacy_desktop_prefs();
    let previous = read(&backup)?;
    let raw = match &previous {
        Some(bytes) => bytes.clone(),
        None => read(&paths.prefs())?.context("Legacy desktop identity is missing")?,
    };
    let identity: Identity =
        serde_json::from_slice(&raw).context("Invalid legacy desktop identity")?;
    let converted = short_id::convert(&identity.client_id)?;
    let current_id = prefs.get::<String>(pref_keys::CLIENT_ID)?;
    ensure!(
        current_id
            .as_deref()
            .is_none_or(|id| id == identity.client_id || id == converted),
        "Legacy desktop identity conflicts with the current identity"
    );
    let seed = identity
        .signature_key_pair
        .as_deref()
        .map(signing_seed)
        .transpose()?;
    if let (Some(legacy), Some(current)) = (
        &seed,
        prefs.get::<String>(pref_keys::SIGNATURE_PRIVATE_KEY)?,
    ) {
        ensure!(
            *legacy == current,
            "Legacy desktop signing key conflicts with the current key"
        );
    }

    if receipt.is_none() {
        let mut source =
            Connection::open_with_flags(source_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let source = source.transaction()?;
        if previous.is_none() {
            let mut file = NamedTempFile::new_in(root)?;
            file.write_all(&raw)?;
            file.as_file().sync_all()?;
            file.persist_noclobber(&backup)?;
        }
        db.with_conn(|conn| -> Result<()> {
            let target = conn.transaction()?;
            let mut assets = Assets::new(root, root, &target).with_desktop_db(&source);
            chat::import(
                &source,
                &target,
                &mut assets,
                &converted,
                chat::Format::Desktop,
            )?;
            let installed = assets.finish();
            plain_libs::kv::set_in_connection(&target, RECEIPT, Some(ROWS_IMPORTED))?;
            target.commit()?;
            installed.committed();
            Ok(())
        })?;
    }

    prefs.update(|values| {
        values.insert(pref_keys::CLIENT_ID.to_owned(), Value::String(converted));
        if let Some(seed) = seed {
            values.insert(
                pref_keys::SIGNATURE_PRIVATE_KEY.to_owned(),
                Value::String(seed),
            );
        }
        Ok(())
    })?;
    db.with_conn(|conn| plain_libs::kv::set_in_connection(conn, RECEIPT, Some(COMPLETE)))?;
    Ok(())
}
