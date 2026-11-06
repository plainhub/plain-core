
    use super::*;

    #[test]
    fn test_cgsize_new() {
        CGSize::new(1.0, 1.0);
        CGSize::new(0.0, -0.0);
        CGSize::new(-0.0, 0.0);
        CGSize::new(-0.0, -0.0);
        CGSize::new(-1.0, -1.0);
        CGSize::new(-1.0, 1.0);
        CGSize::new(1.0, -1.0);
    }

