use super::*;

#[test]
fn test_maybe_parallel_iterator() {
    let mut v = vec![1u32, 2, 3, 4, 5, 6];

    assert_eq!(v.maybe_par_iter().sum::<u32>(), 21);
    assert_eq!(
        v.maybe_par_iter_mut()
            .map(|v| {
                *v *= 2;
                *v
            })
            .sum::<u32>(),
        42
    );
    assert_eq!(v.maybe_par_iter().sum::<u32>(), 42);
    assert_eq!(v.into_maybe_par_iter().sum::<u32>(), 42);
}

#[test]
fn test_maybe_parallel_slice() {
    let v = [1, 2, 3, 4, 5];

    let chunks: Vec<_> = v.maybe_par_chunks(2).collect();
    assert_eq!(chunks, vec![&[1, 2][..], &[3, 4], &[5]]);
}
