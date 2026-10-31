use serde::Deserialize;
use std::collections::{HashMap, HashSet};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Profile {
    #[serde(default)]
    pub abi_order: Vec<String>,
    pub density_dpi: i32,
    pub locale_tags: Vec<String>,
}
#[derive(PartialEq, Eq, Clone, Copy)]
enum Kind {
    Abi,
    Density,
    Language,
    Other,
}
struct Split<'a> {
    name: &'a str,
    module: &'a str,
    config: Option<&'a str>,
}
fn parse(name: &str) -> Split<'_> {
    let file = name.rsplit('/').next().unwrap_or(name);
    let file = file.strip_suffix(".apk").unwrap_or(file);
    let (module, config) = if file == "base" {
        ("base", None)
    } else if let Some(config) = file.strip_prefix("split_config.") {
        ("base", Some(config))
    } else if let Some(rest) = file.strip_prefix("split_") {
        rest.split_once(".config.")
            .filter(|(module, _)| !module.is_empty())
            .map(|(module, config)| (module, Some(config)))
            .unwrap_or((rest, None))
    } else if let Some(module) = file.strip_suffix("-master") {
        (module, None)
    } else {
        file.split_once('-')
            .map(|(module, config)| (module, Some(config)))
            .unwrap_or((file, None))
    };
    Split {
        name,
        module,
        config,
    }
}
fn density(config: &str) -> Option<i32> {
    Some(match config {
        "ldpi" => 120,
        "mdpi" => 160,
        "tvdpi" => 213,
        "hdpi" => 240,
        "xhdpi" => 320,
        "xxhdpi" => 480,
        "xxxhdpi" => 640,
        _ => return None,
    })
}
fn language(config: &str) -> bool {
    let mut parts = config.split(['-', '_']);
    let first = parts.next().unwrap_or_default();
    if !(2..=3).contains(&first.len()) || !first.bytes().all(|c| c.is_ascii_alphabetic()) {
        return false;
    }
    match (parts.next(), parts.next()) {
        (None, None) => true,
        (Some(region), None) => {
            let letters = region
                .strip_prefix('r')
                .or_else(|| region.strip_prefix('R'))
                .unwrap_or(region);
            (region.len() == 3 && region.bytes().all(|c| c.is_ascii_digit()))
                || (letters.len() == 2 && letters.bytes().all(|c| c.is_ascii_alphabetic()))
        }
        _ => false,
    }
}
fn abi(value: &str) -> String {
    value.replace('-', "_")
}
fn kind(config: &str) -> Kind {
    if matches!(
        abi(config).as_str(),
        "arm64_v8a" | "armeabi_v7a" | "armeabi" | "x86_64" | "x86" | "mips" | "mips64" | "riscv64"
    ) {
        Kind::Abi
    } else if config == "nodpi" || density(config).is_some() {
        Kind::Density
    } else if language(config) {
        Kind::Language
    } else {
        Kind::Other
    }
}
fn tag(value: &str) -> String {
    let value = value.to_ascii_lowercase().replace('_', "-");
    if let Some((base, region)) = value.rsplit_once("-r") {
        if region.len() == 2 && region.bytes().all(|c| c.is_ascii_alphabetic()) {
            return format!("{base}-{region}");
        }
    }
    value
}
fn best<'a>(items: &[&Split<'a>], kind: Kind, profile: &Profile) -> Option<&'a str> {
    let selected = match kind {
        Kind::Abi => {
            let by_abi: HashMap<_, _> = items
                .iter()
                .map(|item| (abi(item.config.unwrap()), *item))
                .collect();
            profile
                .abi_order
                .iter()
                .find_map(|value| by_abi.get(&abi(value)).copied())
        }
        Kind::Density => items.iter().copied().min_by_key(|item| {
            let config = item.config.unwrap();
            (
                if config == "nodpi" {
                    0
                } else {
                    (i64::from(density(config).unwrap_or(0)) - i64::from(profile.density_dpi)).abs()
                },
                config == "nodpi",
            )
        }),
        Kind::Language => {
            let by_tag: HashMap<_, _> = items
                .iter()
                .map(|item| (tag(item.config.unwrap()), *item))
                .collect();
            profile
                .locale_tags
                .iter()
                .find_map(|value| by_tag.get(&tag(value)).copied())
                .or_else(|| {
                    profile.locale_tags.iter().find_map(|value| {
                        let value = tag(value);
                        let first = value.split('-').next().unwrap_or_default();
                        items
                            .iter()
                            .copied()
                            .find(|item| tag(item.config.unwrap()).split('-').next() == Some(first))
                    })
                })
                .or_else(|| by_tag.get("en").copied())
        }
        Kind::Other => items.first().copied(),
    };
    selected.map(|item| item.name)
}
pub(super) fn select<'a>(names: &'a [String], profile: &Profile) -> Vec<&'a str> {
    let splits: Vec<_> = names.iter().map(|name| parse(name)).collect();
    let mut modules: Vec<(&str, Vec<&Split<'_>>)> = Vec::new();
    let mut positions = HashMap::new();
    for split in &splits {
        let index = *positions.entry(split.module).or_insert_with(|| {
            modules.push((split.module, Vec::new()));
            modules.len() - 1
        });
        modules[index].1.push(split);
    }
    let mut selected = Vec::new();
    let mut seen = HashSet::new();
    for (_, own) in modules {
        for item in own.iter().filter(|item| item.config.is_none()) {
            if seen.insert(item.name) {
                selected.push(item.name);
            }
        }
        for group in [Kind::Abi, Kind::Density, Kind::Language, Kind::Other] {
            let candidates: Vec<_> = own
                .iter()
                .copied()
                .filter(|item| item.config.is_some_and(|config| kind(config) == group))
                .collect();
            if let Some(name) = best(&candidates, group, profile) {
                if seen.insert(name) {
                    selected.push(name);
                }
            }
        }
    }
    selected
}
pub(super) fn sidecar(name: &str) -> bool {
    matches!(
        name.split('/').next(),
        Some("standalones" | "instant" | "apex")
    )
}

#[cfg(test)]
#[path = "../../../tests/unit/services/packages/splits.rs"]
mod tests;
