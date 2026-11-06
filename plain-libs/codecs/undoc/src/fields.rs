//! Word field instructions shared by the formats that carry them (`.docx` and `.doc`).

/// Where a hyperlink leads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LinkTarget {
    /// An address outside the document: a URL or a file.
    Address(String),
    /// A bookmark in the same document. The output has no anchors to point at, so such a link
    /// is not written as one -- its text is still a link's (see `TextStyle::in_link`).
    Bookmark(String),
}

/// The target of a `HYPERLINK` field instruction: `HYPERLINK "url"`, or `HYPERLINK \l "name"`
/// for a bookmark in the same document.
pub(crate) fn hyperlink_target(instruction: &str) -> Option<LinkTarget> {
    let rest = instruction.trim_start().strip_prefix("HYPERLINK")?;
    let mut url = None;
    let mut bookmark = false;
    let mut chars = rest.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                if chars.next() == Some('l') {
                    bookmark = true;
                }
            }
            '"' => {
                let quoted: String = chars.by_ref().take_while(|&c| c != '"').collect();
                if url.is_none() {
                    url = Some(quoted);
                }
            }
            c if !c.is_whitespace() && url.is_none() => {
                let mut bare = String::from(c);
                while let Some(&next) = chars.peek() {
                    if next.is_whitespace() {
                        break;
                    }
                    bare.push(next);
                    chars.next();
                }
                url = Some(bare);
            }
            _ => {}
        }
    }
    let url = url.filter(|u| !u.is_empty())?;
    Some(if bookmark {
        LinkTarget::Bookmark(url)
    } else {
        LinkTarget::Address(url)
    })
}

#[cfg(test)]
#[path = "../tests/unit/fields.rs"]
mod tests;
