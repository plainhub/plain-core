use super::windows_path::{Component as Utf8WindowsComponent, components as windows_components};
use std::{
    ffi::{OsStr, OsString},
    path::{Component, Path, PathBuf},
};
fn components(name: &str) -> impl Iterator<Item = Utf8WindowsComponent<'_>> {
    windows_components(name)
        .flat_map(|component| match component {
            Utf8WindowsComponent::Normal(s) => split_drive_prefix(s),
            component => [Some(component), None],
        })
        .flatten()
}
fn split_drive_prefix(component: &str) -> [Option<Utf8WindowsComponent<'_>>; 2] {
    let mut rest = component;
    while let [drive, b':', ..] = rest.as_bytes()
        && drive.is_ascii_alphabetic()
    {
        rest = &rest[2..];
    }
    let prefix = windows_components(&component[..component.len() - rest.len()]).next();
    let rest = match rest {
        "" => None,
        "." => Some(Utf8WindowsComponent::CurDir),
        ".." => Some(Utf8WindowsComponent::ParentDir),
        rest => Some(Utf8WindowsComponent::Normal(rest)),
    };
    [prefix, rest]
}
#[cfg(windows)]
pub(crate) fn is_windows_reserved_device_name(name: &str) -> bool {
    let stem = name
        .split(['.', ':'])
        .next()
        .unwrap_or(name)
        .trim_end_matches([' ', '.']);
    if stem.eq_ignore_ascii_case("CON")
        || stem.eq_ignore_ascii_case("PRN")
        || stem.eq_ignore_ascii_case("AUX")
        || stem.eq_ignore_ascii_case("NUL")
        || stem.eq_ignore_ascii_case("CONIN$")
        || stem.eq_ignore_ascii_case("CONOUT$")
        || stem.eq_ignore_ascii_case("CLOCK$")
    {
        return true;
    }
    if stem.len() >= 4 {
        let (prefix, rest) = stem.split_at(3);
        if prefix.eq_ignore_ascii_case("COM") || prefix.eq_ignore_ascii_case("LPT") {
            let mut chars = rest.chars();
            if let Some(c) = chars.next()
                && chars.next().is_none()
                && ((b'1'..=b'9').contains(&(c as u8)) || matches!(c, '¹' | '²' | '³'))
            {
                return true;
            }
        }
    }
    false
}
#[cfg(windows)]
pub(crate) fn is_windows_invalid_component(s: &str) -> bool {
    s.is_empty()
        || s.encode_utf16().count() > 255
        || s.contains(['<', '>', ':', '"', '/', '\\', '|', '?', '*'])
        || s.chars().any(|c| c.is_ascii_control())
        || s.ends_with(['.', ' '])
        || is_windows_reserved_device_name(s)
}
pub(crate) fn file_name_sanitized(no_null_filename: &str) -> PathBuf {
    let clean_components = components(no_null_filename)
        .filter_map(|component| match component {
            Utf8WindowsComponent::Normal(s) => Some(s),
            _ => None,
        })
        .flat_map(|s| s.split(['/', '\\']))
        .flat_map(split_drive_prefix)
        .filter_map(|component| match component {
            Some(Utf8WindowsComponent::Normal(s)) => Some(s),
            _ => None,
        });
    #[cfg(windows)]
    {
        clean_components
            .map(|component| {
                let mut s: String = component
                    .chars()
                    .map(|c| {
                        if matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*')
                            || c.is_ascii_control()
                        {
                            '-'
                        } else {
                            c
                        }
                    })
                    .collect();
                if s.ends_with(['.', ' ']) {
                    let trimmed = s.trim_end_matches(['.', ' ']);
                    if trimmed.is_empty() {
                        s = "_".to_string();
                    } else {
                        s = trimmed.to_string();
                    }
                }
                if is_windows_reserved_device_name(&s) {
                    s = format!("_{s}");
                }
                if s.encode_utf16().count() > 255 {
                    let mut utf16_len = 0;
                    let mut byte_limit = 0;
                    for (idx, ch) in s.char_indices() {
                        let ch_len = ch.len_utf16();
                        if utf16_len + ch_len > 255 {
                            break;
                        }
                        utf16_len += ch_len;
                        byte_limit = idx + ch.len_utf8();
                    }
                    s.truncate(byte_limit);
                    let trimmed = s.trim_end_matches(['.', ' ']);
                    if trimmed.is_empty() {
                        s = "_".to_string();
                    } else {
                        s = trimmed.to_string();
                    }
                    if is_windows_reserved_device_name(&s) {
                        s = format!("_{s}");
                    }
                }
                s
            })
            .collect()
    }
    #[cfg(not(windows))]
    {
        clean_components.collect()
    }
}
pub(crate) fn enclosed_name(file_name: &str) -> Option<PathBuf> {
    let mut depth = 0usize;
    let mut out_path = PathBuf::new();
    for component in components(file_name) {
        match component {
            Utf8WindowsComponent::Prefix | Utf8WindowsComponent::RootDir => {
                if depth > 0 {
                    return None;
                }
            }
            Utf8WindowsComponent::ParentDir => {
                depth = depth.checked_sub(1)?;
                out_path.pop();
            }
            Utf8WindowsComponent::Normal(s) => {
                #[cfg(windows)]
                if s.contains(['/', '\\']) || is_windows_invalid_component(s) {
                    return None;
                }
                #[cfg(not(windows))]
                if s.contains(['/', '\\']) {
                    return None;
                }
                depth += 1;
                out_path.push(s);
            }
            Utf8WindowsComponent::CurDir => (),
        }
    }
    Some(out_path)
}
const MAX_SYMLINK_HOPS: usize = 40;
