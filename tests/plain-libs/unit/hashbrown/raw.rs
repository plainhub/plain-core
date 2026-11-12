use super::*;

#[test]
fn grouped_fold_keeps_every_entry_after_partial_iteration() {
    for len in [0, 1, 3, 7, 8, 9, 15, 16, 17, 64, 257] {
        let mut table = RawTable::new();
        for value in 0..len {
            table.insert(0, value, |_| 0);
        }
        let mut actual = Vec::new();
        let mut iter = unsafe { table.iter() };
        for _ in 0..len / 3 {
            actual.push(unsafe { *iter.next().unwrap().as_ref() });
        }
        actual = iter.fold(actual, |mut values, bucket| {
            values.push(unsafe { *bucket.as_ref() });
            values
        });
        actual.sort_unstable();
        assert_eq!(actual, (0..len).collect::<Vec<_>>());
    }
}

#[test]
#[cfg(debug_assertions)]
fn invalid_group_count_is_rejected_before_any_bucket_callback() {
    let mut table = RawTable::new();
    for value in 0..Group::WIDTH {
        table.insert(0, value, |_| 0);
    }
    let mut iter = unsafe { table.iter() };
    assert!(iter.iter.current_group.0.0.count_ones() > 1);
    iter.items = 1;
    let callbacks = std::cell::Cell::new(0);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        iter.fold((), |(), _| {
            callbacks.set(callbacks.get() + 1);
        });
    }));
    assert!(result.is_err());
    assert_eq!(callbacks.get(), 0);
}

#[test]
fn triangular_probe_visits_every_group_once_and_wraps() {
    for groups in [1, 2, 4, 8, 16, 32, 128] {
        let buckets = groups * Group::WIDTH;
        let mask = buckets - 1;
        for start in [0, 1, Group::WIDTH - 1, mask] {
            let mut sequence = ProbeSeq {
                pos: start,
                stride: 0,
            };
            let mut visited = vec![false; groups];
            for step in 0..groups {
                let group = sequence.pos / Group::WIDTH;
                assert!(!visited[group]);
                visited[group] = true;
                assert_eq!(sequence.pos % Group::WIDTH, start % Group::WIDTH);
                assert_eq!(
                    sequence.pos,
                    (start + Group::WIDTH * step * (step + 1) / 2) & mask
                );
                if step + 1 < groups {
                    sequence.move_next(mask);
                }
            }
            assert!(visited.into_iter().all(|present| present));
        }
    }
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "Went past end of probe sequence")]
fn exhausted_probe_sequence_still_rejects_invalid_advance() {
    let mut sequence = ProbeSeq {
        pos: 0,
        stride: Group::WIDTH,
    };
    sequence.move_next(Group::WIDTH - 1);
}
