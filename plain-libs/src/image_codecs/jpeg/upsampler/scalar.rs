pub fn upsample_horizontal(
    input: &[i16],
    _ref: &[i16],
    _in_near: &[i16],
    _scratch: &mut [i16],
    output: &mut [i16],
) {
    assert_eq!(
        input.len() * 2,
        output.len(),
        "Input length is not half the size of the output length"
    );
    assert!(
        output.len() > 4 && input.len() > 2,
        "Too Short of a vector, cannot upsample"
    );
    output[0] = input[0];
    output[1] = (input[0] * 3 + input[1] + 2) >> 2;
    for (output_window, input_window) in output[2..].chunks_exact_mut(2).zip(input.windows(3)) {
        let sample = 3 * input_window[1] + 2;
        output_window[0] = (sample + input_window[0]) >> 2;
        output_window[1] = (sample + input_window[2]) >> 2;
    }
    let out_len = output.len() - 2;
    let input_len = input.len() - 2;
    let f_out = &mut output[out_len..];
    let i_last = &input[input_len..];
    f_out[0] = (3 * i_last[1] + i_last[0] + 2) >> 2;
    f_out[1] = i_last[1];
}
pub fn upsample_vertical(
    input: &[i16],
    in_near: &[i16],
    in_far: &[i16],
    _scratch_space: &mut [i16],
    output: &mut [i16],
) {
    assert_eq!(input.len() * 2, output.len());
    assert_eq!(in_near.len(), input.len());
    assert_eq!(in_far.len(), input.len());
    let middle = output.len() / 2;
    let (out_top, out_bottom) = output.split_at_mut(middle);
    for ((near, far), x) in input.iter().zip(in_near.iter()).zip(out_top) {
        *x = (((3 * near) + 2) + far) >> 2;
    }
    for ((near, far), x) in input.iter().zip(in_far.iter()).zip(out_bottom) {
        *x = (((3 * near) + 2) + far) >> 2;
    }
}
pub fn upsample_hv(
    input: &[i16],
    in_near: &[i16],
    in_far: &[i16],
    scratch_space: &mut [i16],
    output: &mut [i16],
) {
    assert_eq!(input.len() * 4, output.len());
    assert!(input.len() * 2 <= scratch_space.len());
    let scratch_space = &mut scratch_space[..input.len() * 2];
    let mut t = [0];
    upsample_vertical(input, in_near, in_far, &mut t, scratch_space);
    let scratch_half = scratch_space.len() / 2;
    let output_half = output.len() / 2;
    upsample_horizontal(
        &scratch_space[..scratch_half],
        &[],
        &[],
        &mut t,
        &mut output[..output_half],
    );
    upsample_horizontal(
        &scratch_space[scratch_half..],
        &[],
        &[],
        &mut t,
        &mut output[output_half..],
    );
}
pub fn upsample_generic(
    input: &[i16],
    _in_near: &[i16],
    _in_far: &[i16],
    _scratch_space: &mut [i16],
    output: &mut [i16],
) {
    let difference = output.len() / input.len();
    if difference > 0 {
        for (input, chunk_output) in input.iter().zip(output.chunks_exact_mut(difference)) {
            chunk_output.iter_mut().for_each(|x| *x = *input);
        }
    }
}
