use crate::audio_tags::tag::utils::read_path;

use std::io::{Cursor, Read, Seek, SeekFrom};

#[test]
fn search_for_frame_sync() {
    fn test(data: &[u8], expected_result: Option<u64>) {
        use super::search_for_frame_sync;
        assert_eq!(search_for_frame_sync(&mut &*data).unwrap(), expected_result);
    }

    test(&[0xFF, 0xFB, 0x00], Some(0));
    test(&[0x00, 0x00, 0x01, 0xFF, 0xFB], Some(3));
    test(&[0x01, 0xFF], None);
}

#[test]
	#[rustfmt::skip]
	fn rev_search_for_frame_header() {
		fn test<R: Read + Seek>(reader: &mut R, expected_reader_position: Option<u64>) {
			// We have to start these at the end to do a reverse search, of course :)
			let mut pos = reader.seek(SeekFrom::End(0)).unwrap();

			let ret = super::rev_search_for_frame_header(reader, &mut pos);

			if expected_reader_position.is_some() {
				assert!(ret.is_ok());
				assert!(ret.unwrap().is_some());
				assert_eq!(Some(pos), expected_reader_position);
				return;
			}

			assert!(ret.unwrap().is_none());
		}

		test(&mut Cursor::new([0xFF, 0xFB, 0x52, 0xC4]), Some(0));
		test(&mut Cursor::new([0x00, 0x00, 0x01, 0xFF, 0xFB, 0x52, 0xC4]), Some(3));
		test(&mut Cursor::new([0x01, 0xFF]), None);

		let bytes = read_path(concat!(env!("CARGO_MANIFEST_DIR"), "/../plain-rs/testdata/upstream/lofty/assets/rev_frame_sync_search.mp3"));
		let mut reader = Cursor::new(bytes);
		test(&mut reader, Some(595));
	}
