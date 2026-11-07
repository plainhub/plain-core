#![forbid(unsafe_code)]
mod lower_camel;
mod shouty_snake;
mod snake;
mod upper_camel;
use core::fmt;
pub use lower_camel::{AsLowerCamelCase, ToLowerCamelCase};
pub use shouty_snake::{AsShoutySnakeCase, ToShoutySnakeCase};
pub use snake::{AsSnakeCase, ToSnakeCase};
pub use upper_camel::{AsUpperCamelCase, ToPascalCase, ToUpperCamelCase};

fn transform<F, G>(
    s: &str,
    mut with_word: F,
    mut boundary: G,
    f: &mut fmt::Formatter,
) -> fmt::Result
where
    F: FnMut(&str, &mut fmt::Formatter) -> fmt::Result,
    G: FnMut(&mut fmt::Formatter) -> fmt::Result,
{
    #[derive(Clone, Copy, PartialEq)]
    enum WordMode {
        Boundary,

        Lowercase,

        Uppercase,
    }

    let mut first_word = true;

    for word in s.split(|c: char| !c.is_alphanumeric()) {
        let mut char_indices = word.char_indices().peekable();
        let mut init = 0;
        let mut mode = WordMode::Boundary;

        while let Some((i, c)) = char_indices.next() {
            if let Some(&(next_i, next)) = char_indices.peek() {
                // The mode including the current character, assuming the
                // current character does not result in a word boundary.
                let next_mode = if c.is_lowercase() {
                    WordMode::Lowercase
                } else if c.is_uppercase() {
                    WordMode::Uppercase
                } else {
                    mode
                };

                // Word boundary after if current is not uppercase and next
                // is uppercase
                if next_mode == WordMode::Lowercase && next.is_uppercase() {
                    if !first_word {
                        boundary(f)?;
                    }
                    with_word(&word[init..next_i], f)?;
                    first_word = false;
                    init = next_i;
                    mode = WordMode::Boundary;

                // Otherwise if current and previous are uppercase and next
                // is lowercase, word boundary before
                } else if mode == WordMode::Uppercase && c.is_uppercase() && next.is_lowercase() {
                    if !first_word {
                        boundary(f)?;
                    } else {
                        first_word = false;
                    }
                    with_word(&word[init..i], f)?;
                    init = i;
                    mode = WordMode::Boundary;

                // Otherwise no word boundary, just update the mode
                } else {
                    mode = next_mode;
                }
            } else {
                // Collect trailing characters as a word
                if !first_word {
                    boundary(f)?;
                } else {
                    first_word = false;
                }
                with_word(&word[init..], f)?;
                break;
            }
        }
    }

    Ok(())
}

fn lowercase(s: &str, f: &mut fmt::Formatter) -> fmt::Result {
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == 'Σ' && chars.peek().is_none() {
            write!(f, "ς")?;
        } else {
            write!(f, "{}", c.to_lowercase())?;
        }
    }

    Ok(())
}

fn uppercase(s: &str, f: &mut fmt::Formatter) -> fmt::Result {
    for c in s.chars() {
        write!(f, "{}", c.to_uppercase())?;
    }

    Ok(())
}

fn capitalize(s: &str, f: &mut fmt::Formatter) -> fmt::Result {
    let mut char_indices = s.char_indices();
    if let Some((_, c)) = char_indices.next() {
        write!(f, "{}", c.to_uppercase())?;
        if let Some((i, _)) = char_indices.next() {
            lowercase(&s[i..], f)?;
        }
    }

    Ok(())
}
