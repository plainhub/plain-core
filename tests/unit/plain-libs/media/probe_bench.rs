use super::{cpu_nanoseconds, wav};

use plain_libs::utils::source_reader::{CancellableReader, SourceReader};
use std::io::Write;

#[test]
#[ignore]
#[cfg(unix)]
fn prepared_source_runtime_benchmark() {
    let iterations = std::env::var("PLAIN_ID3_ITERATIONS")
        .map(|value| value.parse::<usize>().unwrap())
        .unwrap_or(20_000);
    for (encoding, name) in [(0, "latin1"), (1, "utf16le"), (2, "utf16be"), (3, "utf8")] {
        for (copies, size) in [(8, "short"), (256, "long")] {
            let text = "Aé".repeat(copies);
            let bytes = wav(4, encoding, &text, false);
            let mut file = plain_libs::temporary::tempfile().unwrap();
            file.write_all(b"prefix").unwrap();
            file.write_all(&bytes).unwrap();
            file.write_all(b"suffix").unwrap();
            let (_signal, stop) = tokio::sync::watch::channel(false);
            let mut reader = SourceReader {
                reader: plain_libs::utils::range_reader::RangeReader::new(
                    file,
                    6,
                    bytes.len() as u64,
                )
                .unwrap(),
                stop,
            };
            let mut total = 0usize;
            let start = std::time::Instant::now();
            let cpu_start = cpu_nanoseconds();
            for _ in 0..iterations {
                let facts = plain_libs::media::probe::probe_audio_source(
                    std::hint::black_box(&mut reader),
                    "fixture.wav",
                );
                assert!(facts.parsed);
                total += std::hint::black_box(
                    facts.title.len() + facts.artist.len() + facts.album.len(),
                );
            }
            let cpu = cpu_nanoseconds() - cpu_start;
            let elapsed = start.elapsed().as_nanos();
            assert_eq!(total, text.len() * 3 * iterations);
            println!(
                "{{\"case\":\"metadata_{name}_{size}\",\"iterations\":{iterations},\"nanoseconds\":{elapsed},\"cpuNanoseconds\":{cpu},\"checksum\":{total}}}"
            );
        }
    }
    let bytes = wav(4, 3, "Source fixture", false);
    let mut file = plain_libs::temporary::tempfile().unwrap();
    file.write_all(&bytes).unwrap();
    let (_signal, stop) = tokio::sync::watch::channel(false);
    let mut source = SourceReader {
        reader: plain_libs::utils::range_reader::RangeReader::new(file, 0, bytes.len() as u64)
            .unwrap(),
        stop,
    };
    let mut file = plain_libs::temporary::tempfile().unwrap();
    file.write_all(&bytes).unwrap();
    let (_file_signal, stop) = tokio::sync::watch::channel(false);
    let mut file = CancellableReader { reader: file, stop };
    for kind in ["source_duration", "file_duration", "source_cover"] {
        let start = std::time::Instant::now();
        let cpu_start = cpu_nanoseconds();
        let mut total = 0u64;
        for _ in 0..iterations {
            total += match kind {
                "source_duration" => plain_libs::media::probe::probe_duration_source(
                    std::hint::black_box(&mut source),
                )
                .unwrap() as u64,
                "file_duration" => {
                    plain_libs::media::probe::probe_duration_file(std::hint::black_box(&mut file))
                        .unwrap() as u64
                }
                "source_cover" => u64::from(
                    plain_libs::media::cover::extract_embedded_cover_source(std::hint::black_box(
                        &mut source,
                    ))
                    .is_none(),
                ),
                _ => unreachable!(),
            };
        }
        let cpu = cpu_nanoseconds() - cpu_start;
        let elapsed = start.elapsed().as_nanos();
        assert_eq!(
            total,
            iterations as u64 * if kind == "source_cover" { 1 } else { 2 }
        );
        println!(
            "{{\"case\":\"{kind}\",\"iterations\":{iterations},\"nanoseconds\":{elapsed},\"cpuNanoseconds\":{cpu},\"checksum\":{total}}}"
        );
    }
}
