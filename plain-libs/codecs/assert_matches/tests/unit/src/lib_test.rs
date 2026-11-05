use std::panic::{catch_unwind, UnwindSafe};

    #[derive(Debug)]
    enum Foo {
        A(i32),
        B(&'static str),
        C(&'static str),
    }

    #[test]
    fn test_assert_succeed() {
        let a = Foo::A(123);

        assert_matches!(a, Foo::A(_));
        assert_matches!(a, Foo::A(123));
        assert_matches!(a, Foo::A(i) if i == 123);
        assert_matches!(a, Foo::A(42) | Foo::A(123));

        let b = Foo::B("foo");

        assert_matches!(b, Foo::B(_));
        assert_matches!(b, Foo::B("foo"));
        assert_matches!(b, Foo::B(s) if s == "foo");
        assert_matches!(b, Foo::B(s) => assert_eq!(s, "foo"));
        assert_matches!(b, Foo::B(s) => { assert_eq!(s, "foo"); assert!(true) });
        assert_matches!(b, Foo::B(s) if s == "foo" => assert_eq!(s, "foo"));
        assert_matches!(b, Foo::B(s) if s == "foo" => { assert_eq!(s, "foo"); assert!(true) });

        let c = Foo::C("foo");

        assert_matches!(c, Foo::B(_) | Foo::C(_));
        assert_matches!(c, Foo::B("foo") | Foo::C("foo"));
        assert_matches!(c, Foo::B(s) | Foo::C(s) if s == "foo");
        assert_matches!(c, Foo::B(s) | Foo::C(s) => assert_eq!(s, "foo"));
        assert_matches!(c, Foo::B(s) | Foo::C(s) => { assert_eq!(s, "foo"); assert!(true) });
        assert_matches!(c, Foo::B(s) | Foo::C(s) if s == "foo" => assert_eq!(s, "foo"));
        assert_matches!(c, Foo::B(s) | Foo::C(s) if s == "foo" => { assert_eq!(s, "foo"); assert!(true) });
    }

    #[test]
    #[should_panic]
    fn test_assert_panic_0() {
        let a = Foo::A(123);

        assert_matches!(a, Foo::B(_));
    }

    #[test]
    #[should_panic]
    fn test_assert_panic_1() {
        let b = Foo::B("foo");

        assert_matches!(b, Foo::B("bar"));
    }

    #[test]
    #[should_panic]
    fn test_assert_panic_2() {
        let b = Foo::B("foo");

        assert_matches!(b, Foo::B(s) if s == "bar");
    }

    #[test]
    #[should_panic]
    fn test_assert_panic_3() {
        let b = Foo::B("foo");

        assert_matches!(b, Foo::B(s) => assert_eq!(s, "bar"));
    }

    #[test]
    #[should_panic]
    fn test_assert_panic_4() {
        let b = Foo::B("foo");

        assert_matches!(b, Foo::B(s) if s == "bar" => assert_eq!(s, "foo"));
    }

    #[test]
    #[should_panic]
    fn test_assert_panic_5() {
        let b = Foo::B("foo");

        assert_matches!(b, Foo::B(s) if s == "foo" => assert_eq!(s, "bar"));
    }

    #[test]
    #[should_panic]
    fn test_assert_panic_6() {
        let b = Foo::B("foo");

        assert_matches!(b, Foo::B(s) if s == "foo" => { assert_eq!(s, "foo"); assert!(false) });
    }

    #[test]
    fn test_assert_no_move() {
        let b = &mut Foo::A(0);
        assert_matches!(*b, Foo::A(0));
    }

    #[test]
    fn assert_with_message() {
        let a = Foo::A(0);

        assert_matches!(a, Foo::A(_), "o noes");
        assert_matches!(a, Foo::A(n) if n == 0, "o noes");
        assert_matches!(a, Foo::A(n) => assert_eq!(n, 0), "o noes");
        assert_matches!(a, Foo::A(n) => { assert_eq!(n, 0); assert!(n < 1) }, "o noes");
        assert_matches!(a, Foo::A(n) if n == 0 => assert_eq!(n, 0), "o noes");
        assert_matches!(a, Foo::A(n) if n == 0 => { assert_eq!(n, 0); assert!(n < 1) }, "o noes");
        assert_matches!(a, Foo::A(_), "o noes {:?}", a);
        assert_matches!(a, Foo::A(n) if n == 0, "o noes {:?}", a);
        assert_matches!(a, Foo::A(n) => assert_eq!(n, 0), "o noes {:?}", a);
        assert_matches!(a, Foo::A(n) => { assert_eq!(n, 0); assert!(n < 1) }, "o noes {:?}", a);
        assert_matches!(a, Foo::A(_), "o noes {value:?}", value=a);
        assert_matches!(a, Foo::A(n) if n == 0, "o noes {value:?}", value=a);
        assert_matches!(a, Foo::A(n) => assert_eq!(n, 0), "o noes {value:?}", value=a);
        assert_matches!(a, Foo::A(n) => { assert_eq!(n, 0); assert!(n < 1) }, "o noes {value:?}", value=a);
        assert_matches!(a, Foo::A(n) if n == 0 => assert_eq!(n, 0), "o noes {value:?}", value=a);
    }

    fn panic_message<F>(f: F) -> String
            where F: FnOnce() + UnwindSafe {
        let err = catch_unwind(f)
            .expect_err("function did not panic");

        *err.downcast::<String>()
            .expect("function panicked with non-String value")
    }

    #[test]
    fn test_panic_message() {
        let a = Foo::A(1);

        // expr, pat
        assert_eq!(panic_message(|| {
            assert_matches!(a, Foo::B(_));
        }), r#"assertion failed: `A(1)` does not match `Foo::B(_)`"#);

        // expr, pat if cond
        assert_eq!(panic_message(|| {
            assert_matches!(a, Foo::B(s) if s == "foo");
        }), r#"assertion failed: `A(1)` does not match `Foo::B(s) if s == "foo"`"#);

        // expr, pat => arm
        assert_eq!(panic_message(|| {
            assert_matches!(a, Foo::B(_) => {});
        }), r#"assertion failed: `A(1)` does not match `Foo::B(_)`"#);

        // expr, pat if cond => arm
        assert_eq!(panic_message(|| {
            assert_matches!(a, Foo::B(s) if s == "foo" => {});
        }), r#"assertion failed: `A(1)` does not match `Foo::B(s) if s == "foo"`"#);

        // expr, pat, args
        assert_eq!(panic_message(|| {
            assert_matches!(a, Foo::B(_), "msg");
        }), r#"assertion failed: `A(1)` does not match `Foo::B(_)`: msg"#);

        // expr, pat if cond, args
        assert_eq!(panic_message(|| {
            assert_matches!(a, Foo::B(s) if s == "foo", "msg");
        }), r#"assertion failed: `A(1)` does not match `Foo::B(s) if s == "foo"`: msg"#);

        // expr, pat => arm, args
        assert_eq!(panic_message(|| {
            assert_matches!(a, Foo::B(_) => {}, "msg");
        }), r#"assertion failed: `A(1)` does not match `Foo::B(_)`: msg"#);

        // expr, pat if cond => arm, args
        assert_eq!(panic_message(|| {
            assert_matches!(a, Foo::B(s) if s == "foo" => {}, "msg");
        }), r#"assertion failed: `A(1)` does not match `Foo::B(s) if s == "foo"`: msg"#);
    }
