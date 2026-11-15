use super::*;

fn cpu_nanoseconds() -> u128 {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    assert_eq!(
        unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) },
        0
    );
    let usage = unsafe { usage.assume_init() };
    (usage.ru_utime.tv_sec as u128 + usage.ru_stime.tv_sec as u128) * 1_000_000_000
        + (usage.ru_utime.tv_usec as u128 + usage.ru_stime.tv_usec as u128) * 1_000
}

fn checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |value, byte| {
        (value ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

fn measure(name: &str, count: usize, mut operation: impl FnMut() -> u64) {
    if std::env::var("PLAIN_IMAGE_CASES")
        .is_ok_and(|cases| !cases.split(',').any(|case| case == name))
    {
        return;
    }
    let expected = operation();
    let start = cpu_nanoseconds();
    for _ in 0..count {
        assert_eq!(std::hint::black_box(operation()), expected);
    }
    let cpu = cpu_nanoseconds() - start;
    println!(
        "{{\"case\":\"{name}\",\"iterations\":{count},\"cpuNanoseconds\":{cpu},\"checksum\":{expected}}}"
    );
}

#[test]
#[ignore]
fn image_io_runtime_benchmark() {
    let count = std::env::var("PLAIN_IMAGE_ITERATIONS")
        .map(|value| value.parse().unwrap())
        .unwrap_or(3000);
    let directory = plain_libs::temporary::tempdir().unwrap();
    for (format, name) in [
        (ImageFormat::Png, "png"),
        (ImageFormat::Jpeg, "jpg"),
        (ImageFormat::Gif, "gif"),
        (ImageFormat::WebP, "webp"),
    ] {
        let bytes = fixture(format, 64, 32);
        let path = directory.path().join(format!("source.{name}"));
        std::fs::write(&path, &bytes).unwrap();
        let path_name = path.to_str().unwrap();
        measure(&format!("thumbnail_{name}"), count, || {
            checksum(&thumbnail(path_name.into(), 32, 16).unwrap().unwrap())
        });
        let output = directory.path().join("converted.png");
        let output_name = output.to_str().unwrap();
        measure(&format!("convert_{name}"), count, || {
            assert_eq!(
                convert(path_name.into(), output_name.into()).unwrap(),
                Value::Bool(true)
            );
            checksum(&std::fs::read(&output).unwrap())
        });
        measure(&format!("file_dimensions_{name}"), count * 5, || {
            let (w, h) = file_dimensions(File::open(&path).unwrap()).unwrap();
            u64::from(w) << 32 | u64::from(h)
        });
        let (_signal, mut reader) = source(&bytes);
        measure(&format!("source_dimensions_{name}"), count * 5, || {
            use std::io::{Seek, SeekFrom};
            reader.seek(SeekFrom::Start(0)).unwrap();
            let (w, h) = source_dimensions(&mut reader).unwrap();
            u64::from(w) << 32 | u64::from(h)
        });
    }
}
