use super::*;

#[test]
fn layers_preserve_order_and_move_owned_payloads_once() {
    let first = layer_fn(|value: usize| value + 3);
    let second = layer_fn(|value: usize| value * 2);
    assert_eq!(second.layer(first.layer(4)), 14);
    assert_eq!(first.layer(second.layer(4)), 11);
    let calls = std::cell::Cell::new(0);
    let layer = layer_fn(|bytes: Box<[u8]>| {
        calls.set(calls.get() + 1);
        bytes
    });
    let bytes: Box<[u8]> = vec![1, 2, 3].into_boxed_slice();
    let pointer = bytes.as_ptr();
    let erased: &dyn Layer<Box<[u8]>, Service = Box<[u8]>> = &layer;
    assert_eq!(erased.layer(bytes).as_ptr(), pointer);
    assert_eq!(calls.get(), 1);
}

#[test]
fn routing_tuple_layers_apply_in_the_original_inner_to_outer_order() {
    let add = layer_fn(|value: usize| value + 3);
    let double = layer_fn(|value: usize| value * 2);
    let subtract = layer_fn(|value: usize| value - 1);
    assert_eq!((add, double).layer(4), 11);
    assert_eq!((add, double, subtract).layer(4), 9);
}
