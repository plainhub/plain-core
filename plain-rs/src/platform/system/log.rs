use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// Log level. Ordered: Trace < Debug < Info < Warn < Error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl Level {
    fn as_str(self) -> &'static str {
        match self {
            Level::Trace => "TRACE",
            Level::Debug => "DEBUG",
            Level::Info => "INFO",
            Level::Warn => "WARN",
            Level::Error => "ERROR",
        }
    }
    fn parse(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "trace" => Level::Trace,
            "debug" => Level::Debug,
            "info" => Level::Info,
            "warn" | "warning" => Level::Warn,
            "error" => Level::Error,
            _ => Level::Info,
        }
    }
    fn to_filter(self) -> log::LevelFilter {
        match self {
            Level::Trace => log::LevelFilter::Trace,
            Level::Debug => log::LevelFilter::Debug,
            Level::Info => log::LevelFilter::Info,
            Level::Warn => log::LevelFilter::Warn,
            Level::Error => log::LevelFilter::Error,
        }
    }
}

/// Initialise the logger and the global level from a string. `RUST_LOG`
/// overrides the explicit `level` argument. Installing the logger a
/// second time (tests) keeps the first installation.
pub fn init(level: &str) -> Result<(), String> {
    static LOGGER: OnceLock<Result<(), String>> = OnceLock::new();
    LOGGER
        .get_or_init(|| log::set_logger(&FILE_LOGGER).map_err(|error| error.to_string()))
        .clone()?;
    let level = match std::env::var("RUST_LOG") {
        Ok(env) => Level::parse(&env),
        Err(_) => Level::parse(level),
    };
    log::set_max_level(level.to_filter());
    Ok(())
}

pub fn init_file(path: &Path, level: &str) -> Result<(), String> {
    set_file(path).map_err(|error| error.to_string())?;
    init(level)
}

#[inline]
pub fn enabled(level: Level) -> bool {
    level.to_filter() <= log::max_level()
}

pub fn metadata_enabled(metadata: &log::Metadata<'_>) -> bool {
    enabled(from_facade_level(metadata.level()))
        && (metadata.level() <= log::Level::Info
            || !matches!(
                metadata.target().split("::").next(),
                Some("fjall" | "lsm_tree")
            ))
}

/// Current level. Tests save/restore it around level-sensitive assertions.
#[cfg_attr(not(test), allow(dead_code))]
pub fn level() -> Level {
    match log::max_level() {
        log::LevelFilter::Trace => Level::Trace,
        log::LevelFilter::Debug => Level::Debug,
        log::LevelFilter::Info => Level::Info,
        log::LevelFilter::Warn => Level::Warn,
        _ => Level::Error,
    }
}

/// Set the level at runtime (config reload, tests). Mirrors it into the
/// facade's max level so `log::` call sites below the threshold skip
/// formatting their arguments entirely.
#[cfg_attr(not(test), allow(dead_code))]
pub fn set_level(level: Level) {
    log::set_max_level(level.to_filter());
}

pub fn write_log(level: Level, body: &str) {
    if !enabled(level) {
        return;
    }
    let mut stderr = std::io::stderr().lock();
    let _ = writeln!(stderr, "{lvl:>5} {body}", lvl = level.as_str());
    append_to_file(level, body);
}

const MAX_LOG_BYTES: u64 = 25 * 1024 * 1024;

pub fn default_log_file(data_dir: &Path) -> PathBuf {
    plain_rs::data_paths::DataPaths::new(data_dir).log_file()
}

struct Sink {
    path: PathBuf,
    file: Option<std::fs::File>,
}

impl Sink {
    fn append(&mut self, line: &str, max_bytes: u64) -> std::io::Result<()> {
        let marker = " [truncated]\n";
        let mut end = line.len().min(max_bytes as usize);
        let truncated = end < line.len();
        if truncated {
            end -= marker.len();
            while !line.is_char_boundary(end) {
                end -= 1;
            }
        }
        let bytes = end as u64 + if truncated { marker.len() as u64 } else { 0 };
        if self.file.is_none() {
            self.file = Some(
                std::fs::File::options()
                    .append(true)
                    .create(true)
                    .open(&self.path)?,
            );
        }
        if self.file.as_ref().unwrap().metadata()?.len() + bytes > max_bytes {
            self.file = None;
            let mut backup = self.path.as_os_str().to_owned();
            backup.push(".bak");
            match std::fs::remove_file(&backup) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
            std::fs::rename(&self.path, &backup)?;
            self.file = Some(
                std::fs::File::options()
                    .append(true)
                    .create(true)
                    .open(&self.path)?,
            );
        }
        let file = self.file.as_mut().unwrap();
        file.write_all(line[..end].as_bytes())?;
        if truncated {
            file.write_all(marker.as_bytes())?;
        }
        file.flush()
    }
}

static SINK: Mutex<Option<Sink>> = Mutex::new(None);

pub fn set_file(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::File::options()
        .append(true)
        .create(true)
        .open(path)?;
    *SINK.lock().unwrap_or_else(|error| error.into_inner()) = Some(Sink {
        path: path.to_path_buf(),
        file: Some(file),
    });
    Ok(())
}

fn append_to_file(level: Level, body: &str) {
    let mut guard = SINK.lock().unwrap_or_else(|error| error.into_inner());
    let Some(sink) = guard.as_mut() else { return };
    let ts = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let line = format!("{ts} {lvl} {body}\n", lvl = level.as_str());
    if let Err(error) = sink.append(&line, MAX_LOG_BYTES) {
        sink.file = None;
        eprintln!("Rust log write failed ({}): {error}", sink.path.display());
    }
}

pub(crate) fn clear_files(paths: &[&Path]) -> io::Result<()> {
    let _guard = SINK.lock().unwrap_or_else(|error| error.into_inner());
    for path in paths {
        for file in log_family(path)? {
            let result = if file == *path {
                std::fs::OpenOptions::new()
                    .write(true)
                    .open(&file)
                    .and_then(|file| file.set_len(0))
            } else {
                std::fs::remove_file(&file)
            };
            match result {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
    }
    Ok(())
}

pub(crate) fn log_family(path: &Path) -> io::Result<Vec<PathBuf>> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("Missing log directory"))?;
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::other("Missing log filename"))?
        .to_string_lossy();
    let prefix = format!("{name}.");
    let entries = match std::fs::read_dir(parent) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry?;
        let entry_name = entry.file_name();
        let entry_name = entry_name.to_string_lossy();
        if (entry_name == name || entry_name.starts_with(&prefix)) && entry.file_type()?.is_file() {
            files.push(entry.path());
        }
    }
    files.sort();
    Ok(files)
}

// ---------------------------------------------------------------------------
// The `log` facade bridge — every call site (NAS modules and plain-rs
// media code alike) logs through the standard `log::` macros and lands
// here: level-filtered, mirrored to stderr, appended to the on-disk sink
// when enabled.
// ---------------------------------------------------------------------------

struct FileLogger;

static FILE_LOGGER: FileLogger = FileLogger;

impl log::Log for FileLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata_enabled(metadata)
    }

    fn log(&self, record: &log::Record) {
        let level = from_facade_level(record.level());
        if !self.enabled(record.metadata()) {
            return;
        }
        write_log(level, &format!("{}", record.args()));
    }

    fn flush(&self) {}
}

fn from_facade_level(level: log::Level) -> Level {
    match level {
        log::Level::Trace => Level::Trace,
        log::Level::Debug => Level::Debug,
        log::Level::Info => Level::Info,
        log::Level::Warn => Level::Warn,
        log::Level::Error => Level::Error,
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/platform/system/log.rs"]
mod tests;
