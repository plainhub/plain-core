use std::hint::black_box;

fn cpu_micros() -> u64 {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    assert_eq!(
        unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) },
        0
    );
    let usage = unsafe { usage.assume_init() };
    (usage.ru_utime.tv_sec as u64 + usage.ru_stime.tv_sec as u64) * 1_000_000
        + usage.ru_utime.tv_usec as u64
        + usage.ru_stime.tv_usec as u64
}

fn measure(name: &str, iterations: usize, mut operation: impl FnMut()) {
    let start = cpu_micros();
    for _ in 0..iterations {
        operation();
    }
    println!("BATCH_BENCH {name} {iterations} {}", cpu_micros() - start);
}

#[test]
#[ignore]
fn dependency_batch_benchmark() {
    let values: Vec<_> = (0..256)
        .map(|number| {
            crate::serde_json::json!({
                "id": number, "text": "synthetic payload with UTF-8 汉字 and escapes \n".repeat(4),
                "float": number as f64 / 7.0, "active": number % 2 == 0
            })
        })
        .collect();
    let input = crate::serde_json::to_vec(&values).unwrap();
    let checksum = input.iter().fold(0_u64, |sum, byte| {
        sum.wrapping_mul(31).wrapping_add(u64::from(*byte))
    });
    println!("BATCH_INPUT {} {checksum}", input.len());
    measure("json_round_trip", 1000, || {
        let value: crate::serde_json::Value =
            crate::serde_json::from_slice(black_box(&input)).unwrap();
        black_box(crate::serde_json::to_vec(black_box(&value)).unwrap());
    });
    let haystack = vec![b'a'; 1024 * 1024];
    measure("byte_search", 10000, || {
        black_box(crate::memchr::memchr(black_box(b'z'), black_box(&haystack)));
        black_box(crate::memchr::memmem::find(
            black_box(&haystack),
            black_box(b"aaaz"),
        ));
    });
    measure("ordered_map", 400, || {
        let mut map = crate::indexmap::IndexMap::<
            u64,
            u64,
            std::hash::BuildHasherDefault<std::collections::hash_map::DefaultHasher>,
        >::default();
        for number in 0..2048 {
            map.insert(black_box(number), number);
        }
        for number in 0..128 {
            black_box(map.shift_remove(&number));
        }
        black_box(
            map.into_iter()
                .fold(0_u64, |sum, (_, value)| sum.wrapping_add(value)),
        );
    });
    let bytes: Vec<_> = (0..32768).map(|number| (number * 97) as u8).collect();
    let encoded = crate::utils::base64::base64_encode(&bytes);
    measure("base64_encode", 2000, || {
        black_box(crate::utils::base64::base64_encode(black_box(&bytes)));
    });
    measure("base64_decode", 1000, || {
        black_box(
            crate::utils::base64::base64_decode_standard(black_box(encoded.as_bytes())).unwrap(),
        );
    });
}
