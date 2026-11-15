use plain_libs::audio_tags::{
    config::ParseOptions,
    file::{FileType, TaggedFileExt},
    probe::Probe,
    tag::Accessor,
};
use std::io::{BufReader, Cursor};

#[test]
fn application_sources_keep_id3_parsers_in_the_library() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let input = directory.path().join("consumer.rs");
    let output = directory.path().join("consumer.ll");
    let owner = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace = owner.parent().unwrap();
    let entries = |name: &str, namespace: &str| {
        std::fs::read_to_string(workspace.join(name))
            .unwrap()
            .split(namespace)
            .skip(1)
            .map(|part| part.split('(').next().unwrap().trim().to_owned())
            .collect::<Vec<_>>()
    };
    let audio = entries(
        "plain-rs/src/features/services/audio/metadata.rs",
        "plain_libs::media::probe::",
    );
    let cover = entries(
        "plain-rs/src/features/services/audio/covers.rs",
        "plain_libs::media::cover::",
    );
    let duration = entries(
        "plain-rs/src/features/services/files/duration_updates.rs",
        "plain_libs::media::probe::",
    );
    let info = entries(
        "plain-rs/src/features/services/files/info.rs",
        "plain_libs::media::probe::",
    );
    assert_eq!(duration.len(), 2);
    assert_eq!(info[0], duration[0]);
    let fixture = include_str!("probe_codegen.rs")
        .replace("AUDIO_ENTRY", &audio[0])
        .replace("FILE_DURATION_ENTRY", &duration[1])
        .replace("DURATION_ENTRY", &duration[0])
        .replace("COVER_ENTRY", &cover[0]);
    std::fs::write(&input, fixture).unwrap();
    let dependencies = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let changed = [
        "src/media/probe.rs",
        "src/media/cover.rs",
        "src/utils/source_reader.rs",
        "src/utils/mod.rs",
    ]
    .map(|name| {
        std::fs::metadata(owner.join(name))
            .unwrap()
            .modified()
            .unwrap()
    })
    .into_iter()
    .max()
    .unwrap();
    let mut libraries: Vec<_> = std::fs::read_dir(&dependencies)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("libplain_libs-")
                && path
                    .extension()
                    .is_some_and(|extension| extension == "rlib")
        })
        .map(|path| (std::fs::metadata(&path).unwrap().modified().unwrap(), path))
        .filter(|(modified, _)| *modified >= changed)
        .collect();
    libraries.sort_by(|left, right| right.0.cmp(&left.0));
    let compiler = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let mut compiled = false;
    for (_, library) in libraries {
        let result = std::process::Command::new(&compiler)
            .args([
                "--edition=2024",
                "--crate-type=lib",
                "--crate-name=consumer",
                "--emit=llvm-ir",
                "-O",
                "-Ccodegen-units=1",
                "--extern",
            ])
            .arg(format!("plain_libs={}", library.display()))
            .arg("-L")
            .arg(format!("dependency={}", dependencies.display()))
            .arg(&input)
            .arg("-o")
            .arg(&output)
            .output()
            .unwrap();
        if !result.status.success()
            && String::from_utf8_lossy(&result.stderr)
                .contains("could not find `media` in `plain_libs`")
        {
            continue;
        }
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        compiled = true;
        break;
    }
    assert!(compiled, "a current media-enabled library is required");
    let ir = std::fs::read_to_string(output).unwrap();
    assert!(
        !ir.contains("UTF-16 string has an invalid byte order mark"),
        "the caller instantiated the ID3 character parser"
    );
    assert!(
        !ir.lines()
            .any(|line| line.starts_with("define ") && line.contains("read_all_frames_into_tag")),
        "the caller instantiated the ID3 frame parser"
    );
}

fn text_bytes(text: &str, encoding: u8) -> Vec<u8> {
    match encoding {
        0 => text.chars().map(|c| c as u8).collect(),
        1 => [
            vec![0xff, 0xfe],
            text.encode_utf16().flat_map(u16::to_le_bytes).collect(),
        ]
        .concat(),
        2 => text.encode_utf16().flat_map(u16::to_be_bytes).collect(),
        3 => text.as_bytes().to_vec(),
        _ => unreachable!(),
    }
}

fn synchsafe(size: usize) -> [u8; 4] {
    [21, 14, 7, 0].map(|shift| ((size >> shift) & 127) as u8)
}

fn wav(version: u8, encoding: u8, text: &str, unsynchronized: bool) -> Vec<u8> {
    let mut frames = Vec::new();
    for (id, old_id) in [(b"TIT2", b"TT2"), (b"TPE1", b"TP1"), (b"TALB", b"TAL")] {
        let mut payload = vec![encoding];
        payload.extend(text_bytes(text, encoding));
        if version == 2 {
            frames.extend(old_id);
            frames.extend(&(payload.len() as u32).to_be_bytes()[1..]);
        } else {
            frames.extend(id);
            frames.extend(if version == 4 {
                synchsafe(payload.len())
            } else {
                (payload.len() as u32).to_be_bytes()
            });
            frames.extend([0, 0]);
        }
        frames.extend(payload);
    }
    if unsynchronized {
        frames = frames
            .into_iter()
            .flat_map(|byte| {
                let mut bytes = vec![byte];
                if byte == 0xff {
                    bytes.push(0);
                }
                bytes
            })
            .collect();
    }
    let mut tag = vec![
        b'I',
        b'D',
        b'3',
        version,
        0,
        if unsynchronized { 128 } else { 0 },
    ];
    tag.extend(synchsafe(frames.len()));
    tag.extend(frames);
    let mut result = b"RIFF\0\0\0\0WAVEfmt ".to_vec();
    result.extend(16u32.to_le_bytes());
    result.extend(1u16.to_le_bytes());
    result.extend(1u16.to_le_bytes());
    result.extend(8_000u32.to_le_bytes());
    result.extend(16_000u32.to_le_bytes());
    result.extend(2u16.to_le_bytes());
    result.extend(16u16.to_le_bytes());
    result.extend(b"id3 ");
    result.extend((tag.len() as u32).to_le_bytes());
    result.extend(&tag);
    if tag.len() % 2 != 0 {
        result.push(0);
    }
    result.extend(b"data");
    result.extend(32u32.to_le_bytes());
    result.extend([0; 32]);
    let size = (result.len() - 8) as u32;
    result[4..8].copy_from_slice(&size.to_le_bytes());
    result
}

fn parse<R: std::io::Read + std::io::Seek>(reader: R) -> plain_libs::audio_tags::file::TaggedFile {
    Probe::with_file_type(reader, FileType::Wav)
        .options(ParseOptions::new().read_properties(false))
        .read()
        .unwrap()
}

#[test]
fn id3_versions_encodings_and_unsynchronization_keep_their_text() {
    for version in [2, 3, 4] {
        for encoding in 0..=3 {
            if version == 2 && encoding > 1 {
                continue;
            }
            for unsynchronized in [false, true] {
                let bytes = wav(version, encoding, "Shared title éÿ", unsynchronized);
                for file in [
                    parse(Cursor::new(bytes.as_slice())),
                    parse(BufReader::new(Cursor::new(bytes.as_slice()))),
                ] {
                    let tag = file.primary_tag().unwrap();
                    assert_eq!(
                        tag.title().as_deref(),
                        Some("Shared title éÿ"),
                        "version={version} encoding={encoding} unsynchronized={unsynchronized}"
                    );
                    assert_eq!(tag.artist().as_deref(), Some("Shared title éÿ"));
                    assert_eq!(tag.album().as_deref(), Some("Shared title éÿ"));
                }
            }
        }
    }
}

#[test]
fn metadata_entry_preserves_id3_versions_encodings_and_duration() {
    for version in [2, 3, 4] {
        for encoding in 0..=3 {
            if version == 2 && encoding > 1 {
                continue;
            }
            for unsynchronized in [false, true] {
                let bytes = wav(version, encoding, "Shared title éÿ", unsynchronized);
                let mut file = plain_libs::temporary::tempfile().unwrap();
                std::io::Write::write_all(&mut file, &bytes).unwrap();
                let (_stop, receiver) = tokio::sync::watch::channel(false);
                let mut source = plain_libs::utils::source_reader::SourceReader {
                    reader: plain_libs::utils::range_reader::RangeReader::new(
                        file,
                        0,
                        bytes.len() as u64,
                    )
                    .unwrap(),
                    stop: receiver,
                };
                for facts in [
                    plain_libs::media::probe::probe_audio_source(&mut source, "fixture.wav"),
                    plain_libs::media::probe::probe_audio_reader_named(
                        &mut Cursor::new(bytes.as_slice()),
                        "fixture.wav",
                    ),
                    plain_libs::media::probe::probe_audio_reader_named(
                        &mut BufReader::new(Cursor::new(bytes.as_slice())),
                        "fixture.wav",
                    ),
                ] {
                    assert!(facts.parsed);
                    assert!(facts.duration_available);
                    assert_eq!(facts.duration_ms, 2);
                    assert_eq!(facts.title, "Shared title éÿ");
                    assert_eq!(facts.artist, "Shared title éÿ");
                    assert_eq!(facts.album, "Shared title éÿ");
                }
            }
        }
    }
}

#[test]
fn prepared_readers_preserve_duration_and_cover_results() {
    let bytes = wav(4, 3, "Duration fixture", false);
    let file = || {
        let mut file = plain_libs::temporary::tempfile().unwrap();
        std::io::Write::write_all(&mut file, &bytes).unwrap();
        file
    };
    let (_stop, receiver) = tokio::sync::watch::channel(false);
    let mut source = plain_libs::utils::source_reader::SourceReader {
        reader: plain_libs::utils::range_reader::RangeReader::new(file(), 0, bytes.len() as u64)
            .unwrap(),
        stop: receiver.clone(),
    };
    let mut direct = plain_libs::utils::source_reader::CancellableReader {
        reader: file(),
        stop: receiver,
    };
    assert_eq!(
        plain_libs::media::probe::probe_duration_source(&mut source),
        Some(2)
    );
    assert_eq!(
        plain_libs::media::probe::probe_duration_file(&mut direct),
        Some(2)
    );
    assert!(plain_libs::media::cover::extract_embedded_cover_source(&mut source).is_none());
}

#[cfg(all(test, unix))]
#[path = "probe_bench.rs"]
mod benchmarks;

#[cfg(unix)]
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
