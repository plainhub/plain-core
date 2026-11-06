
    use alloc::rc::Rc;
    use core::cell::OnceCell;

    use super::*;

    #[test]
    fn return_rc_block() {
        fn get_adder(x: i32) -> RcBlock<dyn Fn(i32) -> i32> {
            RcBlock::new(move |y| y + x)
        }

        let add2 = get_adder(2);
        assert_eq!(add2.call((5,)), 7);
        assert_eq!(add2.call((-1,)), 1);
    }

    #[test]
    fn rc_block_with_precisely_described_lifetimes() {
        fn args<'a, 'b>(
            f: impl Fn(&'a i32, &'b i32) + 'static,
        ) -> RcBlock<dyn Fn(&'a i32, &'b i32) + 'static> {
            RcBlock::new(f)
        }

        fn args_return<'a, 'b>(
            f: impl Fn(&'a i32) -> &'b i32 + 'static,
        ) -> RcBlock<dyn Fn(&'a i32) -> &'b i32 + 'static> {
            RcBlock::new(f)
        }

        fn args_entire<'a, 'b>(f: impl Fn(&'a i32) + 'b) -> RcBlock<dyn Fn(&'a i32) + 'b> {
            RcBlock::new(f)
        }

        fn return_entire<'a, 'b>(
            f: impl Fn() -> &'a i32 + 'b,
        ) -> RcBlock<dyn Fn() -> &'a i32 + 'b> {
            RcBlock::new(f)
        }

        let _ = args(|_, _| {});
        let _ = args_return(|x| x);
        let _ = args_entire(|_| {});
        let _ = return_entire(|| &5);
    }

    #[allow(dead_code)]
    fn covariant<'f>(b: RcBlock<dyn Fn() + 'static>) -> RcBlock<dyn Fn() + 'f> {
        b
    }

    #[test]
    fn allow_re_entrancy() {
        #[allow(clippy::type_complexity)]
        let block: Rc<OnceCell<RcBlock<dyn Fn(u32) -> u32>>> = Rc::new(OnceCell::new());

        let captured_block = block.clone();
        let fibonacci = move |n| {
            let captured_fibonacci = captured_block.get().unwrap();
            match n {
                0 => 0,
                1 => 1,
                n => captured_fibonacci.call((n - 1,)) + captured_fibonacci.call((n - 2,)),
            }
        };

        let block = block.get_or_init(|| RcBlock::new(fibonacci));

        assert_eq!(block.call((0,)), 0);
        assert_eq!(block.call((1,)), 1);
        assert_eq!(block.call((6,)), 8);
        assert_eq!(block.call((10,)), 55);
        assert_eq!(block.call((19,)), 4181);
    }

