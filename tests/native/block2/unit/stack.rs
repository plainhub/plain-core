
    use super::*;

    #[test]
    fn test_size() {
        assert_eq!(
            mem::size_of::<BlockHeader>(),
            <StackBlock<'_, (), (), ()>>::SIZE as _,
        );
        assert_eq!(
            mem::size_of::<BlockHeader>() + mem::size_of::<fn()>(),
            <StackBlock<'_, (), (), fn()>>::SIZE as _,
        );
    }

    #[allow(dead_code)]
    fn covariant<'b, 'f>(
        b: StackBlock<'static, (), (), impl Fn() + 'static>,
    ) -> StackBlock<'b, (), (), impl Fn() + 'f> {
        b
    }

