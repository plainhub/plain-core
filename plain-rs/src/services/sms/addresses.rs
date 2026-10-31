use std::collections::HashSet;

fn normalized(value: &str) -> (String, bool) {
    let value = value.trim();
    let phone = value.chars().any(|c| c.is_numeric())
        && value
            .chars()
            .all(|c| c.is_numeric() || c.is_whitespace() || "+-()./".contains(c));
    if phone {
        (value.chars().filter(|c| c.is_numeric()).collect(), true)
    } else {
        (value.to_lowercase(), false)
    }
}
fn suffix(first: &str, second: &str) -> bool {
    first.chars().count().min(second.chars().count()) >= 7
        && first.chars().count() != second.chars().count()
        && (first.ends_with(second) || second.ends_with(first))
}
pub fn matches(first: &str, second: &str, candidates: &[String]) -> bool {
    let first = normalized(first);
    let second = normalized(second);
    if first == second {
        return true;
    }
    if !first.1 || !second.1 || !suffix(&first.0, &second.0) {
        return false;
    }
    let matches = candidates
        .iter()
        .map(|value| normalized(value))
        .filter(|value| value.1 && suffix(&value.0, &second.0))
        .collect::<HashSet<_>>();
    matches.len() == 1 && matches.contains(&first)
}
pub fn select(candidates: Vec<String>, own: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let candidates = candidates
        .into_iter()
        .filter(|value| !value.trim().is_empty() && seen.insert(normalized(value)))
        .collect::<Vec<_>>();
    let others = candidates
        .iter()
        .filter(|value| !own.iter().any(|own| matches(value, own, &candidates)))
        .cloned()
        .collect::<Vec<_>>();
    if others.is_empty() {
        candidates
    } else {
        others
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MmsAddress {
    pub value: String,
    pub is_sender: bool,
}
pub fn mms(candidates: &[MmsAddress]) -> &str {
    let valid = |value: &&MmsAddress| {
        !value.value.is_empty() && !value.value.eq_ignore_ascii_case("insert-address-token")
    };
    candidates
        .iter()
        .filter(valid)
        .find(|value| value.is_sender)
        .or_else(|| candidates.iter().find(valid))
        .map(|value| value.value.as_str())
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "../../../tests/unit/services/sms/addresses.rs"]
mod tests;
