use crate::xml::{NsReader, Reader, XmlVersion, events::Event, name::ResolveResult};

#[test]
fn document_namespaces_attributes_entities_and_empty_elements_keep_their_meaning() {
    let xml = "<r xmlns='urn:root' xmlns:p='urn:child'><p:c a='A &amp; &#x42;\r\nC'/></r>";
    let mut reader = NsReader::from_str(xml);
    let (namespace, event) = reader.read_resolved_event().unwrap();
    assert!(matches!(namespace, ResolveResult::Bound(n) if n.0 == "urn:root"));
    assert!(matches!(event, Event::Start(e) if e.name().0 == "r"));
    let (namespace, event) = reader.read_resolved_event().unwrap();
    assert!(matches!(namespace, ResolveResult::Bound(n) if n.0 == "urn:child"));
    let Event::Empty(element) = event else {
        panic!("expected empty element")
    };
    let attr = element.attributes().next().unwrap().unwrap();
    assert_eq!(attr.key.0, "a");
    assert_eq!(
        attr.normalized_value(XmlVersion::Implicit1_0).unwrap(),
        "A & B C"
    );
    assert!(matches!(reader.read_event().unwrap(), Event::End(e) if e.name().0 == "r"));
    assert!(matches!(reader.read_event().unwrap(), Event::Eof));
}

#[test]
fn slice_reader_borrows_text_and_element_names_without_copying_the_document() {
    let xml = "<root>independent text</root>";
    let mut reader = Reader::from_str(xml);
    let Event::Start(element) = reader.read_event().unwrap() else {
        panic!("start")
    };
    assert_eq!(element.name().0.as_ptr(), xml[1..].as_ptr());
    let Event::Text(text) = reader.read_event().unwrap() else {
        panic!("text")
    };
    assert_eq!(text.as_ref().as_ptr(), xml[6..].as_ptr());
    assert_eq!(text.as_ref(), "independent text");
    assert_eq!(reader.buffer_position(), 22);
}

#[test]
fn malformed_utf8_mismatched_names_and_duplicate_attributes_remain_errors() {
    let mut reader = Reader::from_reader(&b"<r>\xff</r>"[..]);
    assert!(reader.read_event().is_ok());
    assert!(reader.read_event().is_err());
    for xml in ["<r></x>", "<r><!-- broken --!></r>", "<r a='unterminated>"] {
        let mut reader = Reader::from_str(xml);
        let mut failed = false;
        for _ in 0..xml.len() + 1 {
            match reader.read_event() {
                Err(_) => {
                    failed = true;
                    break;
                }
                Ok(Event::Eof) => break,
                _ => {}
            }
        }
        assert!(failed, "{xml}");
    }
    let mut reader = Reader::from_str("<r a='1' a='2'/>");
    let Event::Empty(element) = reader.read_event().unwrap() else {
        panic!("element")
    };
    let mut attrs = element.attributes();
    assert!(attrs.next().unwrap().is_ok());
    assert!(attrs.next().unwrap().is_err());
}

#[test]
fn namespace_shadowing_does_not_escape_the_element_scope() {
    let mut reader =
        NsReader::from_str("<r xmlns:p='urn:outer'><p:c xmlns:p='urn:inner'/><p:c/></r>");
    reader.read_event().unwrap();
    for expected in ["urn:inner", "urn:outer"] {
        let (namespace, event) = reader.read_resolved_event().unwrap();
        assert!(matches!(event, Event::Empty(_)));
        assert!(matches!(namespace, ResolveResult::Bound(n) if n.0 == expected));
    }
}

#[test]
fn namespace_allocation_limit_rejects_the_first_excess_binding() {
    for count in [128, 129] {
        let mut xml = String::from("<r");
        for index in 0..count {
            xml.push_str(&format!(" xmlns:p{index}='urn:generated:{index}'"));
        }
        xml.push_str("/>");
        let mut reader = NsReader::from_str(&xml);
        let result = reader.read_event();
        if count == 128 {
            assert!(matches!(result.unwrap(), Event::Empty(_)));
        } else {
            assert!(matches!(
                result,
                Err(crate::xml::Error::Namespace(
                    crate::xml::name::NamespaceError::TooManyBindings(128)
                ))
            ));
        }
    }
}

#[test]
#[ignore]
fn benchmark_xml_generated_document() {
    let xml = format!("<r>{}</r>", "<c a='value'>text &amp; text</c>".repeat(8192));
    let start = std::time::Instant::now();
    for _ in 0..64 {
        let mut reader = Reader::from_str(std::hint::black_box(&xml));
        let mut events = 0;
        while reader.buffer_position() < xml.len() as u64 {
            reader.read_event().unwrap();
            events += 1;
        }
        assert_eq!(events, 8192 * 5 + 2);
    }
    println!(
        "XML {} bytes: {:?} per document",
        xml.len(),
        start.elapsed() / 64
    );
}
