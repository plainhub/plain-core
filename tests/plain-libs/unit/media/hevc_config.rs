use super::*;

#[test]
fn array_views_preserve_headers_empty_arrays_and_borrow_nal_bytes() {
    let data = [0xa0, 0, 1, 0, 2, 0x40, 1, 0x21, 0, 0];
    let mut arrays = NalArrays::new(&data, 2);
    let first = arrays.next().unwrap().unwrap();
    assert_eq!(first.bytes(), &data[..7]);
    let nals: Vec<_> = first.nals().collect();
    assert_eq!(nals, [&data[5..7]]);
    assert!(std::ptr::eq(nals[0].as_ptr(), data[5..].as_ptr()));
    assert_eq!(arrays.next().unwrap().unwrap().nals().count(), 0);
    assert!(arrays.next().is_none());
}

#[test]
fn malformed_array_records_return_one_error_without_partial_nals() {
    for data in [
        &[][..],
        &[0x20][..],
        &[0x20, 0, 2, 0, 0][..],
        &[0x20, 0, 1, 0, 3, 0x40][..],
    ] {
        let mut arrays = NalArrays::new(data, 1);
        assert!(arrays.next().unwrap().is_err());
        assert!(arrays.next().is_none());
    }
}

#[test]
fn every_truncation_of_a_complete_record_is_rejected() {
    let data = [0xa0, 0, 1, 0, 2, 0x40, 1];
    for end in 0..data.len() {
        assert!(NalArrays::new(&data[..end], 1).next().unwrap().is_err());
    }
    assert!(NalArrays::new(&data, 1).next().unwrap().is_ok());
}
