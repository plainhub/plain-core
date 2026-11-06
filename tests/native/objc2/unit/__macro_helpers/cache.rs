
    #[test]
    #[should_panic = "class NonExistentClass could not be found"]
    #[cfg(not(any()))]
    fn test_not_found() {
        let _ = crate::class!(NonExistentClass);
    }

