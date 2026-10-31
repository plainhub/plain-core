use super::*;
use std::io::Cursor;

#[test]
fn memory_and_file_ranges_share_bounds_and_integer_reads() {
    let values = [1, 2, 3, 4, 5, 6, 7, 8];
    let mut cursor = Cursor::new(values);
    cursor.set_position(5);
    let reader = ResourceReader::new(cursor).unwrap();
    let file = Bytes::Reader {
        reader: &reader,
        offset: 0,
        length: values.len(),
    };
    let memory = Bytes::Slice(&values);
    for bytes in [file, memory] {
        assert_eq!(bytes.u32(0).unwrap(), 0x04030201);
        assert_eq!(bytes.slice(2..8).unwrap().u16(2).unwrap(), 0x0605);
        assert_eq!(bytes.read(5, 3).unwrap().as_ref(), &[6, 7, 8]);
        assert!(bytes.u32(5).is_err());
        assert!(bytes.read(usize::MAX, 2).is_err());
        assert!(bytes.slice(7..6).is_err());
    }
}

#[test]
fn read_budget_rejects_more_io_before_touching_the_underlying_reader() {
    let reader = ResourceReader::new(Cursor::new([0; 8])).unwrap();
    reader.reader.borrow_mut().get_mut().remaining = 7;
    let bytes = Bytes::Reader {
        reader: &reader,
        offset: 0,
        length: 8,
    };
    assert_eq!(bytes.u32(0).unwrap(), 0);
    assert_eq!(reader.reader.borrow().get_ref().remaining, 0);
    assert!(bytes.u32(4).unwrap_err().to_string().contains("budget"));
    assert_eq!(reader.position.get(), 4);
    assert_eq!(reader.reader.borrow().get_ref().reader.position(), 7);
}

#[test]
fn utf8_and_utf16_string_pools_use_the_same_decoder_for_both_inputs() {
    use super::super::manifest::{Pool, tests::binary};
    for utf8 in [false, true] {
        let document = binary("com.example.package", None, utf8);
        let size = u32::from_le_bytes(document[12..16].try_into().unwrap()) as usize;
        let bytes = &document[8..8 + size];
        let reader = ResourceReader::new(Cursor::new(bytes)).unwrap();
        for input in [
            Bytes::Slice(bytes),
            Bytes::Reader {
                reader: &reader,
                offset: 0,
                length: size,
            },
        ] {
            let pool = Pool::from_bytes(input, 28).unwrap();
            assert_eq!(pool.get(2).unwrap(), "com.example.package");
        }
    }
}

#[test]
fn buffered_random_access_is_limited_by_physical_reads() {
    use std::{
        io::{self, SeekFrom},
        rc::Rc,
    };
    struct Counted {
        position: u64,
        read: Rc<Cell<usize>>,
    }
    impl Read for Counted {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
            bytes.fill(0);
            self.position += bytes.len() as u64;
            self.read.set(self.read.get() + bytes.len());
            Ok(bytes.len())
        }
    }
    impl Seek for Counted {
        fn seek(&mut self, offset: SeekFrom) -> io::Result<u64> {
            self.position = match offset {
                SeekFrom::Start(value) => value,
                SeekFrom::Current(value) => self.position.checked_add_signed(value).unwrap(),
                SeekFrom::End(_) => unreachable!(),
            };
            Ok(self.position)
        }
    }
    let read = Rc::new(Cell::new(0));
    let reader = ResourceReader::new(Counted {
        position: 0,
        read: read.clone(),
    })
    .unwrap();
    let count = READ_LIMIT / 8192 + 1;
    let bytes = Bytes::Reader {
        reader: &reader,
        offset: 0,
        length: count * 16384,
    };
    let mut exhausted = false;
    for index in 0..count {
        if bytes.u32(index * 16384).is_err() {
            exhausted = true;
            break;
        }
    }
    assert!(exhausted, "Read-ahead must consume the resource budget");
    assert!(read.get() <= READ_LIMIT, "read {} bytes", read.get());
}
