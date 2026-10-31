pub fn coordinates(raw: &str) -> Option<(f64, f64)> {
    let raw = raw.trim();
    if raw.len() > 1024 {
        return None;
    }
    let raw = raw.strip_suffix('/').unwrap_or(raw);
    let signs: Vec<usize> = raw
        .char_indices()
        .filter_map(|(index, ch)| matches!(ch, '+' | '-').then_some(index))
        .collect();
    if !matches!(signs.len(), 2 | 3) || signs[0] != 0 {
        return None;
    }
    let mut numbers = Vec::with_capacity(signs.len());
    for (index, start) in signs.iter().copied().enumerate() {
        let part = &raw[start + 1..signs.get(index + 1).copied().unwrap_or(raw.len())];
        if !part.as_bytes().first().is_some_and(u8::is_ascii_digit)
            || !part.bytes().all(|ch| ch.is_ascii_digit() || ch == b'.')
        {
            return None;
        }
        let number: f64 = raw[start..signs.get(index + 1).copied().unwrap_or(raw.len())]
            .parse()
            .ok()?;
        if !number.is_finite() {
            return None;
        }
        numbers.push(number);
    }
    let (latitude, longitude) = (numbers[0], numbers[1]);
    ((-90.0..=90.0).contains(&latitude) && (-180.0..=180.0).contains(&longitude))
        .then_some((latitude, longitude))
}

#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/libs/utils/iso6709.rs"]
mod tests;
