use super::Decoder;
    use crate::{
        bytecast,
        tags::{ByteOrder, Tag, ValueBuffer},
    };

    #[test]
    fn equivalence_of_tag_readers() {
        let file = std::fs::File::open(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../plain-rs/testdata/still-images/rgb.tiff"
        ))
        .unwrap();

        let mut decoder = Decoder::new(file).unwrap();
        let file_bo = decoder.byte_order();
        let mut ifd = decoder.image_ifd();

        {
            let value = ifd
                .find_tag(Tag::BitsPerSample)
                .unwrap()
                .expect("must have BitsPerSample");

            let samples = value.into_u16_vec().unwrap();
            assert_eq!(samples.as_slice(), [8, 8, 8]);
        }

        {
            let mut value = ValueBuffer::empty(crate::tags::Type::SHORT);
            let _entry = ifd
                .find_tag_buf(Tag::BitsPerSample, &mut value)
                .unwrap()
                .expect("must have BitsPerSample");

            value.set_byte_order(ByteOrder::native());
            assert_eq!(value.as_bytes(), bytecast::u16_as_ne_bytes(&[8, 8, 8]));
        }

        {
            let mut by_bytes = [0u16; 4];
            let entry = ifd
                .find_entry(Tag::BitsPerSample)
                .expect("must have BitsPerSample");

            let byte_len = ifd
                .find_tag_bytes(
                    Tag::BitsPerSample,
                    bytecast::u16_as_ne_mut_bytes(&mut by_bytes),
                    0,
                )
                .unwrap()
                .expect("must have BitsPerSample");
            assert_eq!(byte_len, 3 * std::mem::size_of::<u16>());

            file_bo.convert(
                entry.field_type(),
                bytecast::u16_as_ne_mut_bytes(&mut by_bytes[..3]),
                ByteOrder::native(),
            );
            assert_eq!(&by_bytes[..3], &[8, 8, 8]);
        }
    }
