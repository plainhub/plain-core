use super::*;

#[test]
fn file_icons_use_the_same_embedded_bytes_without_starting_the_core() {
    for extension in ["pdf", "folder", "default"] {
        assert_eq!(
            file_icon(extension).as_bytes(),
            EmbeddedBundle
                .read(&format!("ficons/{extension}.svg"))
                .unwrap()
        );
    }
    assert_eq!(file_icon(" .PDF "), file_icon("pdf"));
    for extension in [
        "",
        "unsupported-fixture-extension",
        "../index",
        "folder/../pdf",
        "pdf.svg",
        "pdf%00",
        "pdf\0",
        "💾",
    ] {
        assert_eq!(file_icon(extension), file_icon("default"));
    }
    assert_eq!(file_icon(&"x".repeat(129)), file_icon("default"));
}
