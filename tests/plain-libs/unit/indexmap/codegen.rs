use crate::codegen_test::compile_probe;

#[test]
#[ignore = "Requires PLAIN_LIBS_CODEGEN_RLIB and PLAIN_LIBS_CODEGEN_DEPS from a production library build"]
fn ordered_shifts_inline_hash_lookups_and_use_standard_bucket_references() {
    let ir = compile_probe(
        r#"
extern crate plain_libs;
#[derive(PartialEq, Eq, Hash)]
pub struct Key(pub u64);
type Map = plain_libs::indexmap::IndexMap<Key, u64, std::hash::BuildHasherDefault<std::collections::hash_map::DefaultHasher>>;
#[unsafe(no_mangle)]
pub fn remove_key(map: &mut Map, key: u64) -> Option<u64> { map.shift_remove(&Key(key)) }
#[unsafe(no_mangle)]
pub fn remove_index(map: &mut Map, index: usize) -> Option<(Key, u64)> { map.shift_remove_index(index) }
#[unsafe(no_mangle)]
pub fn move_index(map: &mut Map, from: usize, to: usize) { map.move_index(from, to) }
#[unsafe(no_mangle)]
pub fn insert(map: &mut Map, key: u64, value: u64) -> Option<u64> { map.insert(Key(key), value) }
"#,
    );
    let unexpected: Vec<_> = ir
        .lines()
        .filter(|line| {
            !line.trim_start().starts_with(';')
                && (line.contains("call ") || line.contains("invoke "))
                && ((line.contains("hashbrown")
                    && (line.contains("find_mut")
                        || line.contains("move_next")
                        || line.contains("5entry")))
                    || line.contains("panic_misaligned_pointer_dereference"))
        })
        .collect();
    assert!(
        unexpected.is_empty(),
        "ordered shifts retain lookup/probe/entry wrapper calls or pointer checks: {unexpected:?}"
    );
}
