use super::*;

#[test]
fn deflate_reset_reuses_state_window_and_hash_allocations() {
    let mut encoder = Deflate::new(6, true, 15);
    let state = std::ptr::from_ref(&*encoder.inner.state);
    let window = encoder.inner.state.window.as_ptr();
    let head = encoder.inner.state.head.as_ptr();
    let input = b"independent repeatable compression payload ".repeat(1024);
    let mut output = vec![0; input.len() * 2 + 1024];
    assert_eq!(
        encoder
            .compress(&input, &mut output, DeflateFlush::Finish)
            .unwrap(),
        Status::StreamEnd
    );
    let first = output[..encoder.total_out() as usize].to_vec();
    encoder.reset();
    assert_eq!((encoder.total_in(), encoder.total_out()), (0, 0));
    assert_eq!(std::ptr::from_ref(&*encoder.inner.state), state);
    assert_eq!(encoder.inner.state.window.as_ptr(), window);
    assert_eq!(encoder.inner.state.head.as_ptr(), head);
    assert_eq!(
        encoder
            .compress(&input, &mut output, DeflateFlush::Finish)
            .unwrap(),
        Status::StreamEnd
    );
    assert_eq!(&output[..encoder.total_out() as usize], first);
}
