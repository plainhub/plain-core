use crate::assertions::{_does_impl, assert_impl};

assert_impl!(u32: (Clone) | (Iterator));
assert_impl!(std::rc::Rc<()>: (Clone) | (Send));
assert_impl!(for(T) std::rc::Rc<T>: Clone);

#[test]
fn trait_probes_preserve_true_false_and_boolean_expressions() {
    assert!(_does_impl!(u32: Clone).value());
    assert!(!_does_impl!(std::rc::Rc<()>: Send).value());
    assert!(_does_impl!(std::rc::Rc<()>: (Clone) | (Send)).value());
    assert!(!_does_impl!(std::rc::Rc<()>: (Clone) & (Send)).value());
}
