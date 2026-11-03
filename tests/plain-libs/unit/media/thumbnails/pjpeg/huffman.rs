use super::*;

#[test]
fn canonical_entries_and_decoding_share_one_table_definition() {
    let mut counts = [0; 16];
    counts[1] = 2;
    counts[3] = 1;
    let table = Codebook::new(&counts, &[7, 2, 9]).unwrap();
    assert_eq!(
        table.entries().collect::<Vec<_>>(),
        [(0, 2, 7), (1, 2, 2), (8, 4, 9)]
    );
    let mut bits = Bits::new(&[0b0001_1000], 0);
    assert_eq!(table.symbol(&mut bits).unwrap(), 7);
    assert_eq!(table.symbol(&mut bits).unwrap(), 2);
    assert_eq!(table.symbol(&mut bits).unwrap(), 9);
    assert!(table.symbol(&mut bits).is_err());
}

#[test]
fn invalid_code_counts_are_rejected_before_expansion() {
    assert!(Codebook::new(&[0; 16], &[]).is_err());
    let mut counts = [0; 16];
    counts[0] = 3;
    assert!(Codebook::new(&counts, &[0, 1, 2]).is_err());
    counts[0] = 1;
    assert!(Codebook::new(&counts, &[0, 1]).is_err());
}
