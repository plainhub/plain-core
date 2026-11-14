use super::Pointer;

#[test]
fn pointer_distances_count_elements_for_const_and_mutable_pointers() {
    let mut values = [0_u64; 33];
    let origin = values.as_mut_ptr();
    for index in 0..=values.len() {
        let end = unsafe { origin.add(index) };
        assert_eq!(unsafe { end.distance(origin) }, index);
        assert_eq!(
            unsafe { (end as *const u64).distance(origin.cast_const()) },
            index
        );
        assert_eq!(end.as_usize(), origin.as_usize() + index * size_of::<u64>());
        assert_eq!(end.cast_const().as_usize(), end.as_usize());
    }
}
