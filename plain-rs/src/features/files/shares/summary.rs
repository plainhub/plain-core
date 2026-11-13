use super::SharePath;
use plain_libs::anyhow::Result;
use std::fs;

#[derive(Debug, PartialEq)]
pub struct Summary {
    pub item_count: i64,
    pub total_size: i64,
}

pub fn summarize(roots: &[SharePath]) -> Result<Summary> {
    let mut result = Summary {
        item_count: 0,
        total_size: 0,
    };
    if roots.len() == 1 {
        if let Some(metadata) = metadata(&roots[0].real_path)? {
            if metadata.is_dir() {
                for entry in fs::read_dir(&roots[0].real_path)? {
                    let entry = entry?;
                    if entry.file_name().to_string_lossy().starts_with('.') {
                        continue;
                    }
                    result.add(Some(&entry.metadata()?))?;
                }
            } else {
                result.add(Some(&metadata))?;
            }
        } else {
            result.add(None)?;
        }
    } else {
        for root in roots {
            result.add(metadata(&root.real_path)?.as_ref())?;
        }
    }
    Ok(result)
}

fn metadata(path: &str) -> Result<Option<fs::Metadata>> {
    match fs::metadata(path) {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

impl Summary {
    fn add(&mut self, metadata: Option<&fs::Metadata>) -> Result<()> {
        self.item_count = self
            .item_count
            .checked_add(1)
            .ok_or_else(|| plain_libs::anyhow::anyhow!("Share item count overflow"))?;
        if let Some(metadata) = metadata.filter(|value| !value.is_dir()) {
            let bytes = i64::try_from(metadata.len())?;
            self.total_size = self
                .total_size
                .checked_add(bytes)
                .ok_or_else(|| plain_libs::anyhow::anyhow!("Share size overflow"))?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/files/shares/summary.rs"]
mod tests;
