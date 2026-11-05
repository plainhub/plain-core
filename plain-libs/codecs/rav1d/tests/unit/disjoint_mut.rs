use super::*;

#[test]
fn test_overlapping_immut() {
    let mut v: DisjointMut<Vec<u8>> = Default::default();
    v.resize(10, 0u8);

    let guard1 = v.index(0..5);
    let guard2 = v.index(2..);

    assert_eq!(guard1[2], guard2[0]);
}

#[test]
#[cfg_attr(debug_assertions, should_panic)]
fn test_overlapping_mut() {
    let mut v: DisjointMut<Vec<u8>> = Default::default();
    v.resize(10, 0u8);

    let guard1 = v.index(0..5);
    let mut guard2 = v.index_mut(2..);

    guard2[0] = 42;
    assert_eq!(guard1[2], 42);
}

#[cfg(debug_assertions)]
#[test]
fn test_pointer_write_debug() {
    let mut v: DisjointMut<Vec<[u8; 4]>> = Default::default();
    v.resize(10, [0u8; 4]);

    let guard = v.index(0..);
    let borrow = &guard[..];
    let ptr = v.as_mut_ptr().wrapping_offset(3) as *mut u8;
    // SAFETY: element 3 byte 2 is within the allocated vector and disjoint from the asserted element 4.
    unsafe {
        ptr.wrapping_offset(2).write(42);
    }

    // Miri complains if we dereference the guard after the pointer write here
    // instead of dereferencing and borrowing the slice before the write. I
    // think this is a Miri limitation although to be safe we might want to
    // change how we store the borrow in the guard. This is not an issue in
    // release mode without the guards (see test_pointer_write_release below).
    //
    // assert_eq!(guard[4][0], 0);

    assert_eq!(borrow[4][0], 0);
}

#[cfg(not(debug_assertions))]
#[test]
fn test_pointer_write_release() {
    let mut v: DisjointMut<Vec<[u8; 4]>> = Default::default();
    v.resize(10, [0u8; 4]);

    // SAFETY: the raw write below touches element 3, while this test only reads element 4.
    let borrow = unsafe { v.index(0..) };
    let ptr = v.as_mut_ptr().wrapping_offset(3) as *mut u8;
    // SAFETY: element 3 byte 2 is within the allocated vector and disjoint from the asserted element 4.
    unsafe {
        ptr.wrapping_offset(2).write(42);
    }
    assert_eq!(borrow[4][0], 0);

    // The following triggers UB because the access is through a borrow active
    // during the write.
    //
    // assert_eq!(borrow[3][2], 0);

    // We are fine to re-borrow at this point now that the write is done.
    assert_eq!(unsafe { v.index(4)[0] }, 0);
    assert_eq!(unsafe { v.index(3)[2] }, 42);
}

#[test]
fn test_range_overlap() {
    fn overlaps(a: impl Into<Bounds>, b: impl Into<Bounds>) -> bool {
        let a = a.into();
        let b = b.into();
        a.overlaps(&b)
    }

    // Range overlap.
    assert!(overlaps(5..7, 4..10));
    assert!(overlaps(4..10, 5..7));

    // RangeFrom overlap.
    assert!(overlaps(5.., 4..10));
    assert!(overlaps(4..10, 5..));

    // RangeTo overlap.
    assert!(overlaps(..7, 4..10));
    assert!(overlaps(4..10, ..7));

    // RangeInclusive overlap.
    assert!(overlaps(5..=7, 7..10));
    assert!(overlaps(7..10, 5..=7));

    // RangeToInclusive overlap.
    assert!(overlaps(..=7, 7..10));
    assert!(overlaps(7..10, ..=7));

    // Range no overlap.
    assert!(!overlaps(5..7, 10..20));
    assert!(!overlaps(10..20, 5..7));

    // RangeFrom no overlap.
    assert!(!overlaps(15.., 4..10));
    assert!(!overlaps(4..10, 15..));

    // RangeTo no overlap.
    assert!(!overlaps(..7, 10..20));
    assert!(!overlaps(10..20, ..7));

    // RangeInclusive no overlap.
    assert!(!overlaps(5..=7, 8..10));
    assert!(!overlaps(8..10, 5..=7));

    // RangeToInclusive no overlap.
    assert!(!overlaps(..=7, 8..10));
    assert!(!overlaps(8..10, ..=7));
}
