//! XLSX shared strings parsing.

use std::borrow::Cow;

use crate::decode::{decode_text_lossy, normalize_line_endings, resolve_general_ref};
use crate::error::Result;

/// Shared strings table.
#[derive(Debug, Clone, Default)]
pub struct SharedStrings {
    /// All strings in order
    strings: Vec<String>,
}

impl SharedStrings {
    /// Parse shared strings from XML content.
    pub fn parse(xml: &str) -> Result<Self> {
        let mut strings = Vec::new();
        let mut reader = crate::decode::reader_for(xml);
        // IMPORTANT: Don't trim text - preserve whitespace from xml:space="preserve" elements
        // Excel cells may contain significant leading/trailing spaces
        reader.config_mut().trim_text(false);

        let mut buf = Vec::new();
        let mut in_si = false;
        let mut in_t = false;
        let mut current_text = String::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(plain_libs::xml::events::Event::Start(e)) => match e.name().local_name().as_ref() {
                    "si" => {
                        in_si = true;
                        current_text.clear();
                    }
                    "t" if in_si => {
                        in_t = true;
                    }
                    _ => {}
                },
                Ok(plain_libs::xml::events::Event::Text(e)) if in_t => {
                    let text = decode_text_lossy(&e);
                    current_text.push_str(&text);
                }
                // quick-xml 0.40+ emits entity refs (&amp;, &#13;, …) as separate
                // events; without this arm every entity in a shared string vanishes.
                Ok(plain_libs::xml::events::Event::GeneralRef(e)) if in_t => {
                    current_text.push_str(&resolve_general_ref(&e));
                }
                Ok(plain_libs::xml::events::Event::End(e)) => match e.name().local_name().as_ref() {
                    "si" => {
                        // Collapse CR that re-entered via &#13;/&#xD; refs (Excel
                        // in-cell breaks); a CRLF pair arrives as two refs, so the
                        // whole accumulated string is normalized once here.
                        strings.push(
                            normalize_line_endings(Cow::Borrowed(&current_text)).into_owned(),
                        );
                        in_si = false;
                    }
                    "t" => {
                        in_t = false;
                    }
                    _ => {}
                },
                Ok(plain_libs::xml::events::Event::Eof) => break,
                Err(e) => return Err(e.into()),
                _ => {}
            }
            buf.clear();
        }

        Ok(Self { strings })
    }

    /// Get a string by index.
    pub fn get(&self, index: usize) -> Option<&str> {
        self.strings.get(index).map(|s| s.as_str())
    }

    /// Get the count of shared strings.

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.strings.len()
    }
}

#[cfg(test)]
#[path = "../../tests/unit/xlsx/shared_strings.rs"]
mod tests;
