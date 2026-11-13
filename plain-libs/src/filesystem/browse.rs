use crate::filesystem::SortBy;
use crate::filesystem::record::FileRecord;
use crate::utils::search_dsl::FilterField;
use crate::utils::search_dsl::parse;
use serde::{Deserialize, Serialize};
use std::{
    cmp::Reverse,
    collections::BTreeMap,
    fs, io,
    path::Path,
    time::{Duration, Instant, SystemTime},
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub root: String,
    pub query: String,
    pub sort_by: SortBy,
    pub offset: usize,
    pub limit: Option<usize>,
    pub count_only: bool,
    #[serde(default)]
    pub count_children: bool,
}
pub struct Plan {
    pub root: String,
    show_hidden: bool,
    text: String,
    sizes: Vec<FilterField>,
    recursive: bool,
    sort: SortBy,
    offset: usize,
    limit: Option<usize>,
    count_only: bool,
    count_children: bool,
}
#[derive(Clone, Serialize)]
pub struct Page {
    pub count: usize,
    pub items: Vec<FileRecord>,
}
#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum SortValue {
    Text(String),
    TextDescending(Reverse<String>),
    Number(i64),
    NumberDescending(Reverse<i64>),
}
impl Request {
    pub fn plan(self) -> io::Result<Plan> {
        let fields = parse(&self.query);
        let first = |name: &str| {
            fields
                .iter()
                .find(|f| f.name == name)
                .map(|f| f.value.clone())
                .unwrap_or_default()
        };
        let parent = first("parent");
        let root = if parent.is_empty() { self.root } else { parent };
        if !Path::new(&root).is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "absolute directory path required",
            ));
        }
        let sizes: Vec<_> = fields
            .iter()
            .filter(|f| f.name == "file_size")
            .cloned()
            .collect();
        let text = first("text");
        let recursive = !text.is_empty() || !sizes.is_empty();
        Ok(Plan {
            root,
            show_hidden: first("show_hidden").eq_ignore_ascii_case("true"),
            text: text.to_lowercase(),
            sizes,
            recursive,
            sort: self.sort_by,
            offset: self.offset,
            limit: self.limit,
            count_only: self.count_only,
            count_children: self.count_children,
        })
    }
}
impl Plan {
    pub fn archive_path(&self) -> Option<&str> {
        self.root.split_once("!zip!/").map(|(path, _)| path)
    }
    fn matches(&self, item: &FileRecord, archive: bool) -> bool {
        (archive || item.name.to_lowercase().contains(&self.text))
            && self.sizes.iter().all(|field| {
                if item.is_dir {
                    return false;
                }
                let Ok(value) = field.value.trim().parse::<i64>() else {
                    return false;
                };
                match field.op.as_str() {
                    ">" => item.size > value,
                    ">=" => item.size >= value,
                    "<" => item.size < value,
                    "<=" => item.size <= value,
                    "!=" => item.size != value,
                    "=" | ":" | "" => item.size == value,
                    _ => true,
                }
            })
    }
    pub fn archive_page(&self, mut items: Vec<FileRecord>) -> io::Result<Page> {
        let prefix = format!("{}/", self.root.trim_end_matches('/'));
        if items.iter().any(|item| {
            item.name.is_empty()
                || item.name == "."
                || item.name == ".."
                || item.name.contains('/')
                || item
                    .path
                    .strip_prefix(&prefix)
                    .map(|relative| relative.trim_end_matches('/') != item.name)
                    .unwrap_or(true)
        }) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid archive entry receipt",
            ));
        }
        items.retain(|item| self.matches(item, true));
        Ok(self.page(items))
    }
    pub fn records_page(&self, mut items: Vec<FileRecord>) -> Page {
        items.retain(|item| {
            (self.show_hidden || !item.name.starts_with('.')) && self.matches(item, false)
        });
        self.page(items)
    }
    fn page(&self, mut items: Vec<FileRecord>) -> Page {
        let count = items.len();
        if self.count_only {
            return Page {
                count,
                items: Vec::new(),
            };
        }
        self.sort_items(&mut items);
        Page {
            count,
            items: items
                .into_iter()
                .skip(self.offset)
                .take(self.limit.unwrap_or(usize::MAX))
                .collect(),
        }
    }
    fn sort_key(&self, item: &FileRecord) -> (bool, SortValue, String) {
        let value = match self.sort {
            SortBy::NameAsc => SortValue::Text(item.name.to_lowercase()),
            SortBy::NameDesc => SortValue::TextDescending(Reverse(item.name.to_lowercase())),
            SortBy::DateAsc => SortValue::Number(item.updated_at),
            SortBy::DateDesc => SortValue::NumberDescending(Reverse(item.updated_at)),
            SortBy::SizeAsc => SortValue::Number(item.size),
            SortBy::SizeDesc => SortValue::NumberDescending(Reverse(item.size)),
        };
        (!item.is_dir, value, item.path.clone())
    }
    fn sort_items(&self, items: &mut [FileRecord]) {
        items.sort_by_cached_key(|item| self.sort_key(item));
    }
    pub fn execute(&self) -> io::Result<Page> {
        self.walk_inner(
            None,
            || false,
            Instant::now,
            crate::filesystem::count_dir_entries,
        )
    }
    pub fn walk(
        &self,
        mut progress: impl FnMut(Page) -> bool,
        cancelled: impl Fn() -> bool,
    ) -> io::Result<Page> {
        self.walk_inner(
            Some(&mut progress),
            cancelled,
            Instant::now,
            crate::filesystem::count_dir_entries,
        )
    }
    fn walk_inner(
        &self,
        mut progress: Option<&mut dyn FnMut(Page) -> bool>,
        cancelled: impl Fn() -> bool,
        mut now: impl FnMut() -> Instant,
        mut count_entries: impl FnMut(&Path, bool) -> io::Result<usize>,
    ) -> io::Result<Page> {
        let root = Path::new(&self.root);
        match fs::metadata(root) {
            Ok(meta) if !meta.is_dir() => return Ok(self.page(Vec::new())),
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(self.page(Vec::new())),
            Err(e) => return Err(e),
        }
        let mut directories = vec![fs::read_dir(root)?];
        let mut items = BTreeMap::new();
        let mut count = 0usize;
        let mut visited = 0usize;
        let mut last_progress = None;
        let mut last_count = 0;
        while let Some(directory) = directories.last_mut() {
            if cancelled() {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "File search cancelled",
                ));
            }
            let Some(entry) = directory.next() else {
                directories.pop();
                continue;
            };
            if visited > 0 && visited % 128 == 0 {
                let current = now();
                if let Some(progress) = progress.as_mut().filter(|_| {
                    count != last_count
                        && last_progress.is_none_or(|last| {
                            current.duration_since(last) >= Duration::from_millis(100)
                        })
                }) {
                    let snapshot = Page {
                        count,
                        items: items
                            .values()
                            .skip(self.offset)
                            .take(self.limit.unwrap_or(usize::MAX))
                            .cloned()
                            .collect(),
                    };
                    if !progress(snapshot) {
                        return Err(io::Error::new(
                            io::ErrorKind::Interrupted,
                            "File search cancelled",
                        ));
                    }
                    last_progress = Some(now());
                    last_count = count;
                }
            }
            visited += 1;
            let entry = match entry {
                Ok(entry) => entry,
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::PermissionDenied | io::ErrorKind::NotFound
                    ) =>
                {
                    continue;
                }
                Err(error) => return Err(error),
            };
            let name = entry.file_name().to_string_lossy().into_owned();
            if !self.show_hidden && name.starts_with('.') {
                continue;
            }
            let path = entry.path();
            let meta = match fs::symlink_metadata(&path) {
                Ok(meta) => meta,
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::PermissionDenied | io::ErrorKind::NotFound
                    ) =>
                {
                    continue;
                }
                Err(e) => return Err(e),
            };
            #[cfg(any(target_os = "macos", windows))]
            if !self.show_hidden && super::hidden(&meta) {
                continue;
            }
            if self.recursive && meta.is_dir() {
                match fs::read_dir(&path) {
                    Ok(directory) => directories.push(directory),
                    Err(e)
                        if matches!(
                            e.kind(),
                            io::ErrorKind::PermissionDenied | io::ErrorKind::NotFound
                        ) => {}
                    Err(e) => return Err(e),
                }
            }
            let record = FileRecord {
                name,
                path: path.to_string_lossy().into_owned(),
                permission: String::new(),
                created_at: meta.created().ok().map(millis),
                updated_at: millis(meta.modified()?),
                size: if meta.is_dir() {
                    0
                } else {
                    i64::try_from(meta.len()).map_err(io::Error::other)?
                },
                is_dir: meta.is_dir(),
                child_count: 0,
                file_id: String::new(),
            };
            if self.matches(&record, false) {
                count += 1;
                if !self.count_only {
                    items.insert(self.sort_key(&record), record);
                    if self
                        .limit
                        .is_some_and(|limit| items.len() > self.offset.saturating_add(limit))
                    {
                        items.pop_last();
                    }
                }
            }
        }
        if self.count_only {
            return Ok(Page {
                count,
                items: Vec::new(),
            });
        }
        let mut page = Page {
            count,
            items: items
                .into_values()
                .skip(self.offset)
                .take(self.limit.unwrap_or(usize::MAX))
                .collect(),
        };
        for item in &mut page.items {
            if cancelled() {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "File search cancelled",
                ));
            }
            if self.count_children && item.is_dir {
                item.child_count = match count_entries(Path::new(&item.path), self.show_hidden) {
                    Ok(count) => i32::try_from(count).unwrap_or(i32::MAX),
                    Err(e)
                        if matches!(
                            e.kind(),
                            io::ErrorKind::PermissionDenied | io::ErrorKind::NotFound
                        ) =>
                    {
                        0
                    }
                    Err(e) => return Err(e),
                };
            }
        }
        Ok(page)
    }
}
fn millis(time: SystemTime) -> i64 {
    crate::chrono::DateTime::<crate::chrono::Utc>::from(time).timestamp_millis()
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/filesystem/browse.rs"]
mod tests;
