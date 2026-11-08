use std::cmp::Ordering;
use std::collections::HashMap;
use std::io;
use std::path::{Component, Path};
use std::sync::OnceLock;

// ========================================================================= //

pub struct CaseMapper(HashMap<char, char>);

impl CaseMapper {
    fn new() -> CaseMapper {
        // extracted exceptional uppercase characters from icu_casemap library
        CaseMapper(include!("uppercase.txt").iter().copied().collect())
    }
    fn simple_uppercase(&self, c: char) -> char {
        self.0
            .get(&c)
            .copied()
            .or(c.to_uppercase().next())
            .unwrap_or_default()
    }
}

const MAX_NAME_LEN: usize = 31;

// ========================================================================= //

/// Converts a char to uppercase as defined in MS-CFB,
/// using simple capitalization and the ability to add exceptions.
/// Used when two directory entry names need to be compared.
fn cfb_uppercase_char(c: char) -> char {
    static CASE_MAPPER: OnceLock<CaseMapper> = OnceLock::new();
    let case_mapper = CASE_MAPPER.get_or_init(CaseMapper::new);

    // TODO: Edge cases can be added that appear
    // in the table from Appendix A, <3> Section 2.6.4

    // Base case, just do a simple uppercase
    // equivalent to icu_casemap::CaseMapper::new().simple_uppercase(c)
    case_mapper.simple_uppercase(c)
}

/// The key under which a name is indexed: two names get the same key
/// exactly when `compare_names` calls them equal (same UTF-16 length, same
/// characters once uppercased the CFB way).
pub fn name_key(name: &str) -> (usize, String) {
    (
        name.encode_utf16().count(),
        name.chars().map(cfb_uppercase_char).collect(),
    )
}

/// Compares two directory entry names according to CFB ordering, which is
/// case-insensitive, and which always puts shorter names before longer names,
/// as encoded in UTF-16 (i.e. [shortlex
/// order](https://en.wikipedia.org/wiki/Shortlex_order), rather than
/// dictionary order).
pub fn compare_names(name1: &str, name2: &str) -> Ordering {
    // This ASCII fast-path is important for performance.
    // We saw a 10x speedup for many small streams when comparing pure ascii names.
    // Make sure you run the write benchmark before and after changing this code
    // to not introduce regressions.
    if name1.is_ascii() && name2.is_ascii() {
        match name1.len().cmp(&name2.len()) {
            Ordering::Equal => {
                for (left, right) in name1.bytes().zip(name2.bytes()) {
                    let left = left.to_ascii_uppercase();
                    let right = right.to_ascii_uppercase();
                    match left.cmp(&right) {
                        Ordering::Equal => {}
                        other => return other,
                    }
                }
                Ordering::Equal
            }
            other => other,
        }
    } else {
        match name1
            .encode_utf16()
            .count()
            .cmp(&name2.encode_utf16().count())
        {
            // This is actually not 100% correct -- the MS-CFB spec specifies a
            // particular way of doing the uppercasing on individual UTF-16 code
            // units, along with a list of weird exceptions and corner cases.  But
            // hopefully this is good enough for 99+% of the time.
            Ordering::Equal => {
                let n1 = name1.chars().map(cfb_uppercase_char);
                let n2 = name2.chars().map(cfb_uppercase_char);
                n1.cmp(n2)
            }
            other => other,
        }
    }
}

// ========================================================================= //

pub(super) fn validate_name(name: &str) -> io::Result<()> {
    let len = name.encode_utf16().count();
    if len > MAX_NAME_LEN {
        invalid_input!(
            "Object name cannot be more than {} UTF-16 code units (was {})",
            MAX_NAME_LEN,
            len
        );
    }
    for &chr in &['/', '\\', ':', '!'] {
        if name.contains(chr) {
            invalid_input!("Object name cannot contain {} character", chr);
        }
    }
    Ok(())
}

// ========================================================================= //

/// Given a path within a compound file, turns it into a list of child names
/// descending from the root.  Returns an error if the name is invalid.

pub fn name_chain_from_path(path: &Path) -> io::Result<Vec<&str>> {
    let mut names: Vec<&str> = Vec::new();
    for component in path.components() {
        match component {
            Component::Prefix(_) => {
                invalid_input!("Invalid path (must not have prefix)");
            }
            Component::RootDir => names.clear(),
            Component::CurDir => {}
            Component::ParentDir => {
                if names.pop().is_none() {
                    invalid_input!("Invalid path (must be within root)");
                }
            }
            Component::Normal(osstr) => match osstr.to_str() {
                Some(name) => names.push(name),
                None => invalid_input!("Non UTF-8 path"),
            },
        }
    }
    Ok(names)
}

// ========================================================================= //
