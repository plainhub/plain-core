use super::*;

struct Sparse {
    blocks: Vec<(u64, Vec<u8>)>,
    length: u64,
    position: u64,
    read_bytes: usize,
}

impl Read for Sparse {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let size = (self.length.saturating_sub(self.position)).min(output.len() as u64) as usize;
        let end = self.position + size as u64;
        let mut copied = 0;
        for (start, bytes) in &self.blocks {
            let left = self.position.max(*start);
            let right = end.min(*start + bytes.len() as u64);
            if left < right {
                output[(left - self.position) as usize..(right - self.position) as usize]
                    .copy_from_slice(&bytes[(left - start) as usize..(right - start) as usize]);
                copied += (right - left) as usize;
            }
        }
        if copied != size {
            return Err(io::Error::other("unexpected read of skipped payload"));
        }
        self.position = end;
        self.read_bytes += size;
        Ok(size)
    }
}
impl Seek for Sparse {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let position = match from {
            SeekFrom::Start(n) => i128::from(n),
            SeekFrom::End(n) => i128::from(self.length) + i128::from(n),
            SeekFrom::Current(n) => i128::from(self.position) + i128::from(n),
        };
        self.position =
            u64::try_from(position).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
        Ok(self.position)
    }
}

#[test]
fn co64_samples_above_four_gib_skip_large_unselected_movie_metadata_and_padding() {
    let source = classic_file(false, true);
    let moov = source.windows(4).position(|v| v == b"moov").unwrap() - 4;
    let mut movie = source[moov + 8..].to_vec();
    let media_start = (1u64 << 32) + 128;
    let co64 = movie.windows(4).position(|v| v == b"co64").unwrap() + 4;
    movie[co64 + 8..co64 + 16].copy_from_slice(&(media_start + 8).to_be_bytes());
    movie[co64 + 16..co64 + 24].copy_from_slice(&(media_start + 12).to_be_bytes());
    let hidden_size = 128 * 1024 * 1024u64;
    let movie_end = ftyp().len() as u64 + 8 + hidden_size + movie.len() as u64;
    let prefix = [
        ftyp(),
        words(&[(8 + hidden_size + movie.len() as u64) as u32]),
        b"moov".to_vec(),
        words(&[hidden_size as u32]),
        b"udta".to_vec(),
    ]
    .concat();
    let padding = [
        words(&[1]),
        b"free".to_vec(),
        (media_start - movie_end).to_be_bytes().to_vec(),
    ]
    .concat();
    let mut reader = Sparse {
        blocks: vec![
            (0, prefix),
            (ftyp().len() as u64 + 8 + hidden_size, movie),
            (movie_end, padding),
            (media_start, atom(b"mdat", &[1, 2, 3, 4, 5, 6, 7, 8])),
        ],
        length: media_start + 16,
        position: 0,
        read_bytes: 0,
    };
    let track = AvcTrack::read(&mut reader).unwrap();
    assert_eq!(
        track.read_sample(&mut reader, 4, 2).unwrap(),
        Some(vec![7, 8])
    );
    assert!(reader.read_bytes < 4096, "{}", reader.read_bytes);
}
