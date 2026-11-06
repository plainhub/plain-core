use super::*;

/// A SummaryInformation stream with the given (pid, type, value bytes) properties.
pub(crate) fn stream(props: &[(u32, u16, Vec<u8>)]) -> Vec<u8> {
    let mut out = vec![0xFE, 0xFF, 0, 0];
    out.extend([0u8; 4 + 16]);
    out.extend(1u32.to_le_bytes());
    out.extend([0u8; 16]); // FMTID
    out.extend(48u32.to_le_bytes());
    let mut body = Vec::new();
    let mut table = Vec::new();
    let head = 8 + props.len() * 8;
    for (id, kind, value) in props {
        table.extend(id.to_le_bytes());
        table.extend(((head + body.len()) as u32).to_le_bytes());
        body.extend(kind.to_le_bytes());
        body.extend([0, 0]);
        body.extend(value);
        while body.len() % 4 != 0 {
            body.push(0);
        }
    }
    out.extend(((head + body.len()) as u32).to_le_bytes());
    out.extend((props.len() as u32).to_le_bytes());
    out.extend(table);
    out.extend(body);
    out
}

fn lpstr(bytes: &[u8]) -> Vec<u8> {
    let mut v = ((bytes.len() + 1) as u32).to_le_bytes().to_vec();
    v.extend(bytes);
    v.push(0);
    v
}

#[test]
fn summary_properties_fill_the_metadata() {
    let created: u64 = (1_700_000_000 + 11_644_473_600) * 10_000_000;
    let data = stream(&[
        (PID_CODEPAGE, VT_I2, 1252u16.to_le_bytes().to_vec()),
        (PID_TITLE, VT_LPSTR, lpstr(b"Annual \x93report\x94")),
        (PID_AUTHOR, VT_LPSTR, lpstr(b"A. Writer")),
        (PID_KEYWORDS, VT_LPSTR, lpstr(b"finance; 2023, audit")),
        (PID_CREATED, VT_FILETIME, created.to_le_bytes().to_vec()),
        (PID_PAGE_COUNT, VT_I4, 12i32.to_le_bytes().to_vec()),
    ]);
    let meta = parse(&data);
    assert_eq!(meta.title.as_deref(), Some("Annual \u{201C}report\u{201D}"));
    assert_eq!(meta.author.as_deref(), Some("A. Writer"));
    assert_eq!(meta.keywords, ["finance", "2023", "audit"]);
    assert_eq!(meta.created.as_deref(), Some("2023-11-14T22:13:20Z"));
    assert_eq!(meta.page_count, Some(12));
}

#[test]
fn utf8_code_page_and_empty_strings() {
    let data = stream(&[
        (PID_CODEPAGE, VT_I2, (65001u16).to_le_bytes().to_vec()),
        (PID_TITLE, VT_LPSTR, lpstr("보고서".as_bytes())),
        (PID_SUBJECT, VT_LPSTR, lpstr(b"")),
    ]);
    let meta = parse(&data);
    assert_eq!(meta.title.as_deref(), Some("보고서"));
    assert_eq!(meta.subject, None);
}

#[cfg(feature = "codepages")]
#[test]
fn multi_byte_code_pages_decode_with_tables() {
    // "보고서" in EUC-KR (949).
    let data = stream(&[
        (PID_CODEPAGE, VT_I2, 949u16.to_le_bytes().to_vec()),
        (
            PID_TITLE,
            VT_LPSTR,
            lpstr(&[0xBA, 0xB8, 0xB0, 0xED, 0xBC, 0xAD]),
        ),
    ]);
    assert_eq!(parse(&data).title.as_deref(), Some("보고서"));
}

#[test]
fn a_never_set_time_is_none() {
    assert_eq!(filetime(0), None);
    assert_eq!(
        filetime(116_444_736_000_000_000).as_deref(),
        Some("1970-01-01T00:00:00Z")
    );
}
