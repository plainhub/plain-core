const ATTRIBUTES: &[(u32, &str)] = &include!("data/attributes.rs");
const STYLES: &[(u32, &str)] = &include!("data/styles.rs");
fn lookup(table: &'static [(u32, &str)], id: u32) -> Option<&'static str> {
    table
        .binary_search_by_key(&id, |(key, _)| *key)
        .ok()
        .map(|index| table[index].1)
}
pub(super) fn attribute(id: u32) -> Option<&'static str> {
    lookup(ATTRIBUTES, id)
}
pub(super) fn style(id: u32) -> Option<&'static str> {
    lookup(STYLES, id)
}

#[cfg(test)]
#[path = "../../../tests/unit/services/packages/framework.rs"]
mod tests;
