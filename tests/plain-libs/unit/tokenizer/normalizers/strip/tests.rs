use super::*;
use crate::tokenizer::normalizer::NormalizedString;
use crate::tokenizer::normalizers::Lowercase;
use crate::tokenizer::normalizers::NFKD;
use unicode_normalization_alignments::UnicodeNormalization;

#[test]
fn test_strip_accents() {
    // Unicode combining char
    let original: String = "Me llamó".nfkd().map(|(c, _)| c).collect();
    let normalized = "Me llamo";
    assert_ne!(original, normalized);
    let mut n = NormalizedString::from(original);
    StripAccents.normalize(&mut n).unwrap();
    assert_eq!(&n.get(), &normalized);

    // Ignores regular ascii
    let original = "Me llamo";
    let normalized = "Me llamo";
    assert_eq!(original, normalized);
    let mut n = NormalizedString::from(original);
    StripAccents.normalize(&mut n).unwrap();
    assert_eq!(&n.get(), &normalized);

    // Does not change chinese
    let original: String = "这很简单".nfkd().map(|(c, _)| c).collect();
    let normalized = "这很简单";
    assert_eq!(original, normalized);
    let mut n = NormalizedString::from(original);
    StripAccents.normalize(&mut n).unwrap();
    assert_eq!(&n.get(), &normalized);
}

#[test]
fn test_vietnamese_bug() {
    let original: String = "ậ…".to_string();
    let normalized = "a...".to_string();
    assert_ne!(original, normalized);
    let mut n = NormalizedString::from(original);
    NFKD.normalize(&mut n).unwrap();
    StripAccents.normalize(&mut n).unwrap();
    assert_eq!(&n.get(), &normalized);
    Lowercase.normalize(&mut n).unwrap();
    assert_eq!(&n.get(), &normalized);

    let original: String = "Cụ thể, bạn sẽ tham gia một nhóm các giám đốc điều hành tổ chức, các nhà lãnh đạo doanh nghiệp, các học giả, chuyên gia phát triển và tình nguyện viên riêng biệt trong lĩnh vực phi lợi nhuận…".to_string();
    let normalized = "cu the, ban se tham gia mot nhom cac giam đoc đieu hanh to chuc, cac nha lanh đao doanh nghiep, cac hoc gia, chuyen gia phat trien va tinh nguyen vien rieng biet trong linh vuc phi loi nhuan...".to_string();
    let mut n = NormalizedString::from(original);
    NFKD.normalize(&mut n).unwrap();
    StripAccents.normalize(&mut n).unwrap();
    Lowercase.normalize(&mut n).unwrap();
    assert_eq!(&n.get(), &normalized);
}

#[test]
fn test_thai_bug() {
    let original = "ำน\u{e49}ำ3ลำ".to_string();
    let normalized = "านา3ลา".to_string();
    assert_ne!(original, normalized);
    let mut n = NormalizedString::from(original);
    NFKD.normalize(&mut n).unwrap();
    StripAccents.normalize(&mut n).unwrap();
    Lowercase.normalize(&mut n).unwrap();
    assert_eq!(&n.get(), &normalized);
}

#[test]
fn test_strip_accents_multiple() {
    let original = "e\u{304}\u{304}\u{304}o";
    let normalized = "eo";
    assert_ne!(original, normalized);
    let mut n = NormalizedString::from(original);
    StripAccents.normalize(&mut n).unwrap();
    assert_eq!(&n.get(), &normalized);
    assert_eq!(
        n,
        NormalizedString::new(
            original.to_string(),
            normalized.to_string(),
            vec![(0, 1), (7, 8)],
            0
        )
    );
    assert_eq!(
        n.alignments_original(),
        vec![
            (0, 1),
            (1, 1),
            (1, 1),
            (1, 1),
            (1, 1),
            (1, 1),
            (1, 1),
            (1, 2)
        ]
    );
}
