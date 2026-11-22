use crate::features::client_identity::MIGRATED_ID_BASE;
use plain_libs::anyhow::{Result, ensure};
use plain_libs::utils::uuid::Uuid;

pub fn convert(value: &str) -> Result<String> {
    if value.len() == 36
        && let Ok(id) = Uuid::parse_str(value)
    {
        return Ok(id.into_string());
    }
    ensure!(
        !matches!(value, "" | "me" | "local")
            && (value == "0" || !value.starts_with('0'))
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || byte.is_ascii_lowercase()),
        "Invalid legacy short UUID"
    );
    let number = u128::from(u64::from_str_radix(value, 36)?);
    ensure!(
        number <= i64::MAX as u128,
        "Legacy short UUID exceeds 63 bits"
    );
    let low = number & ((1u128 << 62) - 1);
    let high = (number >> 62) << 64;
    Ok(Uuid::from_u128(MIGRATED_ID_BASE | high | low).into_string())
}

pub(super) fn optional(value: &str) -> Result<Option<String>> {
    (!value.is_empty()).then(|| convert(value)).transpose()
}

pub fn reference(value: &str) -> Result<String> {
    match value {
        "" | "me" | "local" => Ok(value.into()),
        _ => convert(value),
    }
}

#[cfg(test)]
fn reverse(value: &str) -> Option<String> {
    let id = Uuid::parse_str(value).ok()?.as_u128();
    if id & crate::features::client_identity::MIGRATED_ID_MASK != MIGRATED_ID_BASE {
        return None;
    }
    let mut number = ((id >> 64) & 1) << 62 | (id & ((1u128 << 62) - 1));
    let mut digits = Vec::new();
    loop {
        digits.push(b"0123456789abcdefghijklmnopqrstuvwxyz"[(number % 36) as usize] as char);
        number /= 36;
        if number == 0 {
            return Some(digits.into_iter().rev().collect());
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/features/legacy_upgrade/short_id.rs"]
mod tests;
