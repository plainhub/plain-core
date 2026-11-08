use der::{
    Decode, Header, Length, Reader, Sequence, SliceReader, Tag, Tagged,
    asn1::{AnyRef, BitStringRef, IntRef, ObjectIdentifier, SequenceRef},
};
use pkcs12::cms::cert::x509::{
    Version,
    spki::{AlgorithmIdentifierRef, SubjectPublicKeyInfoRef},
    time::Validity,
};
use std::fmt::Write;

mod names;

#[derive(Sequence)]
struct Certificate<'a> {
    tbs: TbsCertificate<'a>,
    signature_algorithm: AlgorithmIdentifierRef<'a>,
    signature: BitStringRef<'a>,
}

#[derive(Sequence)]
struct TbsCertificate<'a> {
    #[asn1(context_specific = "0", default = "Default::default")]
    version: Version,
    serial_number: IntRef<'a>,
    signature: AlgorithmIdentifierRef<'a>,
    issuer: AnyRef<'a>,
    validity: Validity,
    subject: AnyRef<'a>,
    subject_public_key_info: SubjectPublicKeyInfoRef<'a>,
    #[asn1(context_specific = "1", tag_mode = "IMPLICIT", optional = "true")]
    issuer_unique_id: Option<BitStringRef<'a>>,
    #[asn1(context_specific = "2", tag_mode = "IMPLICIT", optional = "true")]
    subject_unique_id: Option<BitStringRef<'a>>,
    #[asn1(context_specific = "3", optional = "true")]
    extensions: Option<AnyRef<'a>>,
}

#[derive(Sequence)]
struct Extension<'a> {
    oid: ObjectIdentifier,
    #[asn1(default = "Default::default")]
    critical: bool,
    value: AnyRef<'a>,
}

pub(super) fn names(bytes: &[u8]) -> der::Result<(String, String)> {
    let certificate = Certificate::from_der(bytes)?;
    if let Some(extensions) = certificate.tbs.extensions {
        let sequence: &SequenceRef = extensions.decode_as()?;
        let mut reader = SliceReader::new(sequence.as_bytes())?;
        while !reader.is_finished() {
            Extension::decode(&mut reader)?
                .value
                .tag()
                .assert_eq(Tag::OctetString)?;
        }
    }
    let subject = display_name(certificate.tbs.subject.decode_as()?)?;
    let issuer = if certificate.tbs.subject == certificate.tbs.issuer {
        subject.clone()
    } else {
        display_name(certificate.tbs.issuer.decode_as()?)?
    };
    Ok((subject, issuer))
}

fn display_name(name: &SequenceRef) -> der::Result<String> {
    let mut reader = SliceReader::new(name.as_bytes())?;
    let mut display = String::new();
    let mut valid_text = true;
    while !reader.is_finished() {
        if !display.is_empty() {
            display.push_str(", ");
        }
        let header = Header::decode(&mut reader)?;
        header.tag().assert_eq(Tag::Set)?;
        let mut rdn = SliceReader::new(reader.read_slice(header.length())?)?;
        if rdn.is_finished() {
            return Err(Tag::Set.value_error().into());
        }
        let mut first = true;
        while !rdn.is_finished() {
            if !first {
                display.push_str(" + ");
            }
            first = false;
            let element = <&SequenceRef>::decode(&mut rdn)?;
            let mut attribute = SliceReader::new(element.as_bytes())?;
            let oid = ObjectIdentifier::decode(&mut attribute)?;
            let (tag, value) = attribute_value(&mut attribute)?;
            attribute.finish()?;
            match names::short_name(&oid) {
                Some(short) => display.push_str(short),
                None => write!(display, "OID({oid})").unwrap(),
            }
            display.push('=');
            match tag {
                // Keep the text and hex forms used for existing PKCS#12 name matching and alias order.
                7 | 12 | 18..=22 | 25..=27 => match std::str::from_utf8(value) {
                    Ok(value) => display.push_str(value),
                    Err(_) => valid_text = false,
                },
                30 => {
                    let (pairs, trailing) = value.as_chunks::<2>();
                    valid_text &= trailing.is_empty();
                    for character in
                        char::decode_utf16(pairs.iter().copied().map(u16::from_be_bytes))
                    {
                        match character {
                            Ok(character) => display.push(character),
                            Err(_) => valid_text = false,
                        }
                    }
                }
                _ => {
                    for byte in value {
                        write!(display, "{byte:02X}").unwrap();
                    }
                }
            }
        }
    }
    Ok(if valid_text {
        display
    } else {
        "<X509Error: Invalid X.509 name>".into()
    })
}

fn attribute_value<'a>(reader: &mut SliceReader<'a>) -> der::Result<(u8, &'a [u8])> {
    let tag = reader
        .peek_byte()
        .ok_or_else(|| der::Error::incomplete(reader.input_len()))?;
    // Read name strings as bytes, including legacy tags absent from DER's typed strings.
    if matches!(tag, 7 | 12 | 18..=22 | 25..=28 | 30) {
        reader.read_byte()?;
        let length = Length::decode(reader)?;
        return Ok((tag, reader.read_slice(length)?));
    }
    let value = AnyRef::decode(reader)?;
    Ok((tag, value.value()))
}
