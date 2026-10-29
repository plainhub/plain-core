use super::*;

pub(crate) struct Mount {
    pub id: String,
    pub device: String,
    pub root: String,
    pub path: PathBuf,
    pub fs_type: String,
    pub source: String,
}

fn decode(value: &str) -> String {
    value
        .replace("\\040", " ")
        .replace("\\011", "\t")
        .replace("\\012", "\n")
        .replace("\\134", "\\")
}

pub(crate) fn parse(input: &str) -> Result<Vec<Mount>> {
    input
        .lines()
        .map(|line| {
            let (left, right) = line
                .split_once(" - ")
                .context("Invalid mount information")?;
            let fields = left.split_whitespace().collect::<Vec<_>>();
            let fields_after = right.split_whitespace().collect::<Vec<_>>();
            ensure!(
                fields.len() >= 6 && fields_after.len() >= 2,
                "Incomplete mount information"
            );
            Ok(Mount {
                id: fields[0].into(),
                device: fields[2].into(),
                root: decode(fields[3]),
                path: PathBuf::from(decode(fields[4])),
                fs_type: fields_after[0].into(),
                source: decode(fields_after[1]),
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "../../../tests/unit/platform/volumes/mounts.rs"]
mod tests;
