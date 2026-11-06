use super::*;

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

#[test]
fn conventional_parts_are_returned_untouched() {
    let xml = format!(
        r#"<w:document xmlns:w="{W}" xmlns:r="{R}"><w:p><w:r><w:t>hi &amp; bye</w:t></w:r></w:p></w:document>"#
    );
    assert!(!needs_rewrite(&xml));
    assert_eq!(canonical_prefixes(xml.clone()), xml);
}

#[test]
fn an_unconventional_prefix_is_rewritten_for_elements_and_attributes() {
    let xml = format!(
        r#"<ns0:document xmlns:ns0="{W}" xmlns:ns1="{R}"><ns0:p><ns0:hyperlink ns1:id="rId4"><ns0:r><ns0:t>x &lt; y</ns0:t></ns0:r></ns0:hyperlink></ns0:p></ns0:document>"#
    );
    let out = canonical_prefixes(xml);
    assert!(out.contains("<w:document "), "{out}");
    assert!(out.contains(r#"<w:hyperlink r:id="rId4">"#), "{out}");
    assert!(
        out.contains("<w:t>x &lt; y</w:t></w:r></w:hyperlink>"),
        "{out}"
    );
    assert!(out.ends_with("</w:p></w:document>"), "{out}");
    assert!(!out.contains("ns0:p"), "{out}");
}

#[test]
fn a_default_namespace_gets_the_conventional_prefix() {
    let xml = format!(r#"<document xmlns="{W}"><body><p w="1"/></body></document>"#);
    let out = canonical_prefixes(xml);
    assert_eq!(
        out,
        format!(r#"<w:document xmlns="{W}"><w:body><w:p w="1"/></w:body></w:document>"#)
    );
}

#[test]
fn a_conventional_prefix_bound_elsewhere_is_moved_aside() {
    // `w` is the VML-like foreign namespace here; the word namespace is `ns0`.
    let xml = format!(
        r#"<ns0:document xmlns:ns0="{W}" xmlns:w="urn:example:other"><w:t>not text</w:t><ns0:t>text</ns0:t></ns0:document>"#
    );
    let out = canonical_prefixes(xml);
    assert!(out.contains("<_w:t>not text</_w:t>"), "{out}");
    assert!(out.contains("<w:t>text</w:t>"), "{out}");
}

#[test]
fn text_comments_and_cdata_are_preserved() {
    let xml = format!(
        r#"<?xml version="1.0"?><ns0:a xmlns:ns0="{W}"><!-- c --><ns0:t xml:space="preserve">  a  </ns0:t><![CDATA[<x>]]></ns0:a>"#
    );
    let out = canonical_prefixes(xml);
    assert!(out.starts_with(r#"<?xml version="1.0"?><w:a "#), "{out}");
    assert!(out.contains("<!-- c -->"), "{out}");
    assert!(
        out.contains(r#"<w:t xml:space="preserve">  a  </w:t>"#),
        "{out}"
    );
    assert!(out.contains("<![CDATA[<x>]]>"), "{out}");
}

#[test]
fn malformed_input_is_returned_as_written() {
    let xml = format!(r#"<ns0:a xmlns:ns0="{W}"><ns0:b></ns0:a>"#);
    assert_eq!(canonical_prefixes(xml.clone()), xml);
}
