use super::*;

/// A Word picture with an SVG original and an HD Photo layer, the way Office writes it.
const SVG_PICTURE: &str = r#"<w:drawing><wp:inline><wp:docPr id="1" name="Picture 1" descr="A red &amp; blue chart"/>
<a:graphic><a:graphicData><pic:pic><pic:nvPicPr><pic:cNvPr id="1" name="chart.png"/></pic:nvPicPr>
<pic:blipFill><a:blip r:embed="rIdPng"><a:extLst>
<a:ext uri="{BEBA8EAE-BF5A-486C-A8C5-ECC9F3942E4B}"><a14:imgProps><a14:imgLayer r:embed="rIdWdp"/></a14:imgProps></a:ext>
<a:ext uri="{96DAC541-7B7A-43D3-8B79-37D633B846F1}"><asvg:svgBlip r:embed="rIdSvg"/></a:ext>
</a:extLst></a:blip></pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing>
<w:drawing><wp:inline><wp:docPr id="2" name="Picture 2"/><a:graphic><a:graphicData><pic:pic>
<pic:blipFill><a:blip r:embed="rIdPlain"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing>"#;

#[test]
fn companions_belong_to_their_picture() {
    let refs = scan_pictures(SVG_PICTURE);
    assert_eq!(
        refs.companions.get("rIdSvg"),
        Some(&(ResourceRole::Alternate, "rIdPng".to_string()))
    );
    assert_eq!(
        refs.companions.get("rIdWdp"),
        Some(&(ResourceRole::Layer, "rIdPng".to_string()))
    );
    assert!(!refs.companions.contains_key("rIdPng"));
    assert!(!refs.companions.contains_key("rIdPlain"));
    assert_eq!(refs.referenced, ["rIdPng", "rIdWdp", "rIdSvg", "rIdPlain"]);
}

/// A shape's description describes that shape's picture; a picture bullet has none, and a
/// described shape without a picture does not lend its description to the next picture.
#[test]
fn description_does_not_leak_to_bullets_or_later_pictures() {
    let xml = r#"<p:spTree>
<p:sp><p:nvSpPr><p:cNvPr id="2" name="Body" descr="A list"/></p:nvSpPr><p:txBody><a:p><a:pPr><a:buBlip><a:blip r:embed="rIdBullet"/></a:buBlip></a:pPr></a:p></p:txBody></p:sp>
<p:sp><p:nvSpPr><p:cNvPr id="3" name="Label" descr="Only a label"/></p:nvSpPr></p:sp>
<p:pic><p:nvPicPr><p:cNvPr id="4" name="Picture 3"/></p:nvPicPr><p:blipFill><a:blip r:embed="rIdPlain"/></p:blipFill></p:pic>
<p:sp><p:nvSpPr><p:cNvPr id="5" name="Card" descr="Card art"/></p:nvSpPr><p:spPr><a:blipFill><a:blip r:embed="rIdFill"/></a:blipFill></p:spPr></p:sp>
</p:spTree>"#;
    let refs = scan_pictures(xml);
    assert_eq!(refs.referenced, ["rIdBullet", "rIdPlain", "rIdFill"]);
    assert_eq!(refs.alt_texts.get("rIdBullet"), None);
    assert_eq!(refs.alt_texts.get("rIdPlain"), None);
    assert_eq!(
        refs.alt_texts.get("rIdFill").map(String::as_str),
        Some("Card art")
    );
}

#[test]
fn description_goes_to_the_picture_that_has_it() {
    let refs = scan_pictures(SVG_PICTURE);
    assert_eq!(
        refs.alt_texts.get("rIdPng").map(String::as_str),
        Some("A red & blue chart")
    );
    assert_eq!(
        refs.alt_texts.get("rIdPlain"),
        None,
        "picture 2 has no descr"
    );
}
