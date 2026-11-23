mod assets;
mod chat;
mod preferences;
mod room;
mod rows;
mod short_id;

use crate::features::prefs::{Prefs, pref_keys};
use crate::infrastructure::db::Db;
use plain_libs::anyhow::{Context, Result, ensure};
use plain_libs::rusqlite::{Connection, OptionalExtension};
use plain_libs::serde_json::{self, Value};
use plain_libs::utils::base64::{base64_decode_checked, base64_encode};
use serde::{Deserialize, Serialize};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

const STATE_KEY: &str = "legacy_data_upgrade_3_3_25";

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Status {
    Unavailable,
    Pending,
    Skipped,
    Imported,
    Cleared,
}

#[derive(Serialize, Deserialize)]
struct Receipt {
    status: Status,
    files: Vec<PathBuf>,
    pomodoro_settings: Option<Value>,
}

impl Receipt {
    fn read(connection: &Connection) -> Result<Option<Self>> {
        let value: Option<String> = connection
            .query_row(
                "SELECT value FROM key_values WHERE key=?1",
                [STATE_KEY],
                |row| row.get(0),
            )
            .optional()?;
        Ok(value.as_deref().map(serde_json::from_str).transpose()?)
    }

    fn write(&self, connection: &Connection) -> Result<()> {
        Ok(plain_libs::kv::set_in_connection(
            connection,
            STATE_KEY,
            Some(&serde_json::to_string(self)?),
        )?)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LegacyStorage {
    database_directory: PathBuf,
    preferences_directory: PathBuf,
    files_directory: PathBuf,
}

impl LegacyStorage {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            [
                &self.database_directory,
                &self.preferences_directory,
                &self.files_directory
            ]
            .into_iter()
            .all(|path| path.is_absolute()),
            "Absolute legacy storage roots required"
        );
        Ok(())
    }

    fn database(&self) -> PathBuf {
        self.database_directory.join("plain.db")
    }

    pub(crate) fn preferences(&self) -> PathBuf {
        self.preferences_directory
            .join("datastore/settings.preferences_pb")
    }

    pub(crate) fn backup_preferences(&self) -> PathBuf {
        self.preferences().with_extension("preferences_pb.bak")
    }

    fn present(&self, target_db: &Path) -> Result<bool> {
        let source_db = self.database();
        ensure!(
            source_db != target_db,
            "Legacy database overlaps the current database"
        );
        if target_db.try_exists()? && source_db.try_exists()? {
            ensure!(
                !plain_libs::utils::file_identity::same(&source_db, target_db)?,
                "Legacy database overlaps the current database"
            );
        }
        Ok(source_db.try_exists()? || self.backup_preferences().try_exists()?)
    }

    pub fn prepare_identity(&self, prefs: &Prefs) -> Result<bool> {
        let path = self.preferences();
        if !path.try_exists()? {
            return Ok(false);
        }
        let backup = self.backup_preferences();
        ensure!(
            !backup.try_exists()?,
            "Legacy settings backup already exists"
        );
        let values = preferences::read(&path)?;
        if let Some(old_id) = values.client_id.as_deref() {
            let id = short_id::convert(old_id)?;
            let signing_key = values
                .signature_key_pair
                .as_deref()
                .map(convert_keypair)
                .transpose()?;
            prefs.update(|values| {
                values.insert(pref_keys::CLIENT_ID.to_owned(), Value::String(id));
                if let Some(signing_key) = signing_key {
                    values.insert(
                        pref_keys::SIGNATURE_KEY_PAIR.to_owned(),
                        Value::String(signing_key),
                    );
                }
                Ok(())
            })?;
        }
        std::fs::rename(&path, &backup).context("Failed to back up legacy settings")?;
        Ok(true)
    }

    pub fn status(&self, db: &Db, target: &Path) -> Result<Status> {
        if let Some(receipt) = db.with_conn(|conn| Receipt::read(conn))? {
            return Ok(receipt.status);
        }
        Ok(if self.present(&target.join("plain.db"))? {
            Status::Pending
        } else {
            Status::Unavailable
        })
    }

    pub fn import(&self, db: &Db, prefs: &Prefs, target: &Path) -> Result<Status> {
        let _app_files = db.app_files_lock()?;
        let receipt = db.with_conn(|conn| self.import_rows(conn, prefs, target))?;
        if let Some(mut receipt) = receipt {
            if let Some(settings) = receipt.pomodoro_settings.take() {
                prefs.update(|entries| {
                    entries
                        .entry(pref_keys::POMODORO_SETTINGS.to_owned())
                        .or_insert(settings);
                    Ok(())
                })?;
            }
            receipt.status = Status::Imported;
            db.with_conn(|conn| receipt.write(conn))?;
            Ok(receipt.status)
        } else {
            self.status(db, target)
        }
    }

    fn import_rows(
        &self,
        conn: &mut Connection,
        prefs: &Prefs,
        target: &Path,
    ) -> Result<Option<Receipt>> {
        if let Some(receipt) = Receipt::read(conn)? {
            match receipt.status {
                Status::Imported | Status::Cleared => return Ok(None),
                Status::Pending => return Ok(Some(receipt)),
                _ => {}
            }
        }
        ensure!(
            self.present(&target.join("plain.db"))?,
            "Legacy data is unavailable"
        );
        let mut settings = preferences::read(&self.backup_preferences())?;
        let self_id = prefs
            .get::<String>(pref_keys::CLIENT_ID)?
            .unwrap_or_default();
        if let Some(id) = &settings.client_id {
            ensure!(
                short_id::convert(id)? == self_id,
                "Legacy device identity was not initialized"
            );
        }
        if prefs.get::<Value>(pref_keys::POMODORO_SETTINGS)?.is_some() {
            settings.pomodoro_settings = None;
        }
        let tx = conn.transaction()?;
        let mut assets = assets::Assets::new(&self.files_directory, target, &tx);
        if self.database().try_exists()? {
            let mut source = room::read(&self.database())?;
            let source = source.transaction()?;
            rows::import(&source, &tx, &mut assets)?;
            chat::import(&source, &tx, &mut assets, &self_id)?;
        }
        let pomodoro_settings = rows::import_preferences(settings, &tx, &mut assets)?;
        let (files, installed) = assets.finish();
        let receipt = Receipt {
            status: Status::Pending,
            files,
            pomodoro_settings,
        };
        complete_onboarding(&tx)?;
        receipt.write(&tx)?;
        tx.commit()?;
        installed.committed();
        Ok(Some(receipt))
    }

    pub fn clear(&self, db: &Db, target: &Path) -> Result<Status> {
        let _app_files = db.app_files_lock()?;
        db.with_conn(|conn| {
            let mut receipt = Receipt::read(conn)?.context("Legacy data has not been imported")?;
            if receipt.status == Status::Cleared {
                return Ok(receipt.status);
            }
            ensure!(
                receipt.status == Status::Imported,
                "Legacy data has not been imported"
            );
            self.present(&target.join("plain.db"))?;
            for relative in &receipt.files {
                ensure!(
                    assets::clean_relative(relative),
                    "Invalid migrated file path"
                );
                match assets::source_path(&self.files_directory, relative) {
                    Ok(path) => std::fs::remove_file(path)?,
                    Err(error)
                        if error
                            .downcast_ref::<std::io::Error>()
                            .is_some_and(|e| e.kind() == ErrorKind::NotFound) => {}
                    Err(error) => return Err(error),
                }
            }
            if self.database().try_exists()? {
                room::clear(&self.database())?;
            }
            preferences::clear(&self.backup_preferences())?;
            receipt.files.clear();
            receipt.status = Status::Cleared;
            receipt.write(conn)?;
            Ok(receipt.status)
        })
    }
}

fn complete_onboarding(conn: &Connection) -> Result<()> {
    Ok(plain_libs::kv::set_in_connection(
        conn,
        pref_keys::ONBOARDING_COMPLETED,
        Some("true"),
    )?)
}

fn convert_keypair(json: &str) -> Result<String> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Keypair {
        private_key: String,
        public_key: String,
    }

    let keypair: Keypair = plain_libs::serde_json::from_str(json)?;
    let private = base64_decode_checked(&keypair.private_key)?;
    let public = base64_decode_checked(&keypair.public_key)?;
    ensure!(
        private.len() == 32 && public.len() == 32,
        "Invalid legacy signing key length"
    );
    let mut combined = private;
    combined.extend(&public);
    ensure!(
        plain_libs::crypto::ed25519_public_from_keypair(&combined).is_some(),
        "Invalid legacy signing keypair"
    );
    Ok(base64_encode(&combined))
}

#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/features/legacy_upgrade/import.rs"]
mod tests;
