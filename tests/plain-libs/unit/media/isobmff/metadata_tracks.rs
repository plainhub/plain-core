use super::*;
use std::sync::atomic::Ordering;

fn spans(bytes: &[u8]) -> io::Result<Vec<Span>> {
    let mut result = Vec::new();
    let mut offset = 0;
    while offset < bytes.len() {
        let span = span(bytes, offset)?;
        offset = span.end;
        result.push(span);
    }
    Ok(result)
}

fn atom(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    [
        ((payload.len() + 8) as u32).to_be_bytes().as_slice(),
        kind,
        payload,
    ]
    .concat()
}

fn track(id: u32, codec: &[u8; 4]) -> Vec<u8> {
    let mut header = vec![0; 16];
    header[12..16].copy_from_slice(&id.to_be_bytes());
    let description = [1u32.to_be_bytes().as_slice(), atom(codec, &[]).as_slice()].concat();
    let description = atom(b"stsd", &[vec![0; 4], description].concat());
    let offsets = atom(
        b"stco",
        &[
            vec![0; 4],
            1u32.to_be_bytes().to_vec(),
            20u32.to_be_bytes().to_vec(),
        ]
        .concat(),
    );
    atom(
        b"trak",
        &[
            atom(b"tkhd", &header),
            atom(
                b"mdia",
                &atom(b"minf", &atom(b"stbl", &[description, offsets].concat())),
            ),
        ]
        .concat(),
    )
}

fn indexed(kind: &[u8; 4], id: u32) -> Vec<u8> {
    atom(kind, &[vec![0; 4], id.to_be_bytes().to_vec()].concat())
}

fn transform(source: &[u8]) -> Vec<u8> {
    let mut output = Cursor::new(Vec::new());
    assert!(
        remove_mebx_tracks(
            &mut Cursor::new(source),
            &mut output,
            &AtomicBool::new(false)
        )
        .unwrap()
    );
    output.into_inner()
}

#[test]
fn metadata_removal_keeps_media_bytes_chunk_offsets_and_file_layout_identical() {
    let video = track(1, b"avc1");
    let movie = atom(b"moov", &[video.clone(), track(2, b"mebx")].concat());
    let media = atom(b"mdat", &[1, 2, 3, 4, 5, 6]);
    for boxes in [vec![movie.clone(), media.clone()], vec![media, movie]] {
        let source = boxes.concat();
        let output = transform(&source);
        assert_eq!(source.len(), output.len());
        for span in spans(&source).unwrap() {
            if &span.kind == b"moov" {
                let movie = &output[span.data..span.end];
                let children = spans(movie).unwrap();
                assert_eq!(&movie[children[0].start..children[0].end], video);
                assert_eq!(&children[1].kind, b"free");
                assert!(
                    movie[children[1].data..children[1].end]
                        .iter()
                        .all(|byte| *byte == 0)
                );
            } else {
                assert_eq!(source[span.start..span.end], output[span.start..span.end]);
            }
        }
    }
}

#[test]
fn fragmented_metadata_references_are_retired_without_changing_audio_video_fragments() {
    let defaults = atom(
        b"mvex",
        &[indexed(b"trex", 1), indexed(b"trex", 2)].concat(),
    );
    let movie = atom(
        b"moov",
        &[track(1, b"avc1"), track(2, b"mebx"), defaults].concat(),
    );
    let video_fragment = atom(b"traf", &indexed(b"tfhd", 1));
    let fragments = atom(
        b"moof",
        &[video_fragment.clone(), atom(b"traf", &indexed(b"tfhd", 2))].concat(),
    );
    let random_access = atom(
        b"mfra",
        &[indexed(b"tfra", 1), indexed(b"tfra", 2)].concat(),
    );
    let source = [
        movie,
        fragments,
        atom(b"mdat", &[7; 512]),
        random_access,
        indexed(b"sidx", 2),
    ]
    .concat();
    let output = transform(&source);
    assert_eq!(source.len(), output.len());
    for span in spans(&output).unwrap() {
        let bytes = &output[span.data..span.end];
        match &span.kind {
            b"moov" => {
                let defaults = child(bytes, b"mvex").unwrap();
                let entries = spans(defaults).unwrap();
                assert_eq!(&entries[0].kind, b"trex");
                assert_eq!(&entries[1].kind, b"free");
            }
            b"moof" => {
                let entries = spans(bytes).unwrap();
                assert_eq!(&bytes[entries[0].start..entries[0].end], video_fragment);
                assert_eq!(&entries[1].kind, b"free");
            }
            b"mfra" => {
                let entries = spans(bytes).unwrap();
                assert_eq!(&entries[0].kind, b"tfra");
                assert_eq!(&entries[1].kind, b"free");
            }
            b"mdat" => assert_eq!(bytes, &[7; 512]),
            b"free" => assert!(bytes.iter().all(|byte| *byte == 0)),
            _ => panic!("Unexpected box"),
        }
    }
}

#[test]
fn non_metadata_files_do_not_copy_or_rewrite_output() {
    let source = atom(b"moov", &track(1, b"avc1"));
    let mut output = Cursor::new(Vec::new());
    assert!(
        !remove_mebx_tracks(
            &mut Cursor::new(source),
            &mut output,
            &AtomicBool::new(false)
        )
        .unwrap()
    );
    assert!(output.into_inner().is_empty());
}

#[test]
fn malformed_and_cancelled_sources_fail_before_output_is_written() {
    for source in [
        vec![0; 7],
        atom(b"moov", &[0; 7]),
        atom(b"mdat", &[0; 8]),
        atom(b"moov", &[track(1, b"avc1"), track(1, b"mebx")].concat()),
    ] {
        let mut output = Cursor::new(Vec::new());
        assert!(
            remove_mebx_tracks(
                &mut Cursor::new(source),
                &mut output,
                &AtomicBool::new(false)
            )
            .is_err()
        );
        assert!(output.into_inner().is_empty());
    }
    let mut output = Cursor::new(Vec::new());
    assert!(
        remove_mebx_tracks(
            &mut Cursor::new(atom(b"moov", &track(2, b"mebx"))),
            &mut output,
            &AtomicBool::new(true)
        )
        .is_err()
    );
    assert!(output.into_inner().is_empty());
}

#[test]
fn cancellation_during_streaming_stops_before_the_next_copy_block() {
    struct Output<'a> {
        bytes: Cursor<Vec<u8>>,
        cancelled: &'a AtomicBool,
    }
    impl Write for Output<'_> {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            let written = self.bytes.write(bytes)?;
            self.cancelled.store(true, Ordering::Release);
            Ok(written)
        }
        fn flush(&mut self) -> io::Result<()> {
            self.bytes.flush()
        }
    }
    impl Seek for Output<'_> {
        fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
            self.bytes.seek(from)
        }
    }
    let source = [
        atom(b"mdat", &vec![1; 128 * 1024]),
        atom(b"moov", &track(2, b"mebx")),
    ]
    .concat();
    let cancelled = AtomicBool::new(false);
    let mut output = Output {
        bytes: Cursor::new(Vec::new()),
        cancelled: &cancelled,
    };
    assert!(remove_mebx_tracks(&mut Cursor::new(source), &mut output, &cancelled).is_err());
    assert_eq!(output.bytes.into_inner().len(), 64 * 1024);
}
