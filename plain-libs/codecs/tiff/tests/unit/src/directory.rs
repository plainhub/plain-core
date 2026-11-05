use super::Directory;
    use crate::{decoder::ifd::Entry, tags::Tag};

    #[test]
    fn directory_multiple_entries() {
        let mut dir = Directory::empty();
        assert_eq!(dir.len(), 0);

        dir.extend((0..=u16::MAX).map(|i| {
            let tag = Tag::Unknown(1);
            let entry = Entry::new_u64(crate::tags::Type::BYTE, i.into(), [0; 8]);
            (tag, entry)
        }));

        assert_eq!(dir.len(), 1, "Only one tag was ever modified");

        assert_eq!(
            dir.get(Tag::Unknown(1))
                .expect("tag 1 should be present after this chain")
                .count(),
            u16::MAX.into()
        );
    }

    #[test]
    fn iteration_order() {
        let mut dir = Directory::empty();
        assert_eq!(dir.len(), 0);

        let fake_entry = Entry::new_u64(crate::tags::Type::BYTE, 0, [0; 8]);
        dir.extend((0..32).map(|i| {
            let tag = Tag::Unknown(i);
            let entry = fake_entry.clone();
            (tag, entry)
        }));

        let iter_order: Vec<u16> = dir.iter().map(|(tag, _e)| tag.to_u16()).collect();
        assert_eq!(
            iter_order,
            (0..32).collect::<Vec<_>>(),
            "Tags must be in ascending order according to the specification"
        );
    }
