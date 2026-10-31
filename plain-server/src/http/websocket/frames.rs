use crate::http::auth::replay::Replay;

pub(super) fn admit<'a>(
    replay: &Replay,
    client: &str,
    frame: &[u8],
    plain: &'a [u8],
    registered: bool,
    now: i64,
) -> Result<&'a [u8], ()> {
    let (timestamp, payload) = if registered {
        (
            i64::from_be_bytes(plain.get(..8).ok_or(())?.try_into().map_err(|_| ())?),
            plain.get(8..).ok_or(())?,
        )
    } else {
        (
            std::str::from_utf8(plain)
                .map_err(|_| ())?
                .parse::<i64>()
                .map_err(|_| ())?,
            &[][..],
        )
    };
    let nonce = plain_rs::utils::hex::bytes_to_hex(frame.get(..24).ok_or(())?);
    replay.admit(client, &format!("{timestamp}|{nonce}|"), now)?;
    Ok(payload)
}

#[cfg(test)]
#[path = "../../../../tests/plain-server/unit/http/websocket/frames.rs"]
mod tests;
