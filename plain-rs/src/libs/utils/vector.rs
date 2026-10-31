pub fn decode_f32_be(bytes: &[u8]) -> Result<Vec<f32>, String> {
    if bytes.is_empty() || bytes.len() % 4 != 0 || bytes.len() > 4096 * 4 {
        return Err("invalid embedding dimensions".into());
    }
    let values = bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_be_bytes(chunk.try_into().unwrap()))
        .collect::<Vec<_>>();
    if values.iter().any(|value| !value.is_finite()) {
        return Err("invalid embedding values".into());
    }
    Ok(values)
}
