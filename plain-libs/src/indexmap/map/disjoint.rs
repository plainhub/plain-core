use crate::indexmap::GetDisjointMutError;
pub(super) fn get_disjoint_mut<T, const N: usize>(
    entries: &mut [T],
    indices: [usize; N],
) -> Result<[&mut T; N], GetDisjointMutError> {
    let len = entries.len();
    for i in 0..N {
        let idx = indices[i];
        if idx >= len {
            return Err(GetDisjointMutError::IndexOutOfBounds);
        } else if indices[..i].contains(&idx) {
            return Err(GetDisjointMutError::OverlappingIndices);
        }
    }
    let entries_ptr = entries.as_mut_ptr();
    Ok(indices.map(move |idx| unsafe { &mut *(entries_ptr.add(idx)) }))
}
#[track_caller]
pub(super) fn get_disjoint_opt_mut<T, const N: usize>(
    entries: &mut [T],
    indices: [Option<usize>; N],
) -> [Option<&mut T>; N] {
    let len = entries.len();
    for i in 0..N {
        if let Some(idx) = indices[i] {
            if idx >= len {
                unreachable!("`get_index_of` returned an out-of-bounds index");
            } else if indices[..i].contains(&Some(idx)) {
                panic!("duplicate keys found");
            }
        }
    }
    let entries_ptr = entries.as_mut_ptr();
    indices.map(move |idx_opt| match idx_opt {
        Some(idx) => Some(unsafe { &mut *entries_ptr.add(idx) }),
        None => None,
    })
}
