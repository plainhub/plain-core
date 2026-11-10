use plain_rs::infrastructure::db::Db;
pub(crate) struct Replay {
    scope: &'static str,
}
impl Replay {
    pub(crate) fn new(scope: &'static str) -> Self {
        Self { scope }
    }
    pub(crate) fn admit<'a>(
        &self,
        db: &Db,
        client: &str,
        plain: &'a str,
        now: i64,
    ) -> Result<&'a str, ()> {
        let mut parts = plain.splitn(3, '|');
        let timestamp = parts.next().ok_or(())?.parse::<i64>().map_err(|_| ())?;
        let nonce = parts.next().ok_or(())?;
        let body = parts.next().ok_or(())?;
        let hex = |n: usize| nonce.len() == n && nonce.bytes().all(|c| c.is_ascii_hexdigit());
        let uuid = nonce.len() == 36
            && nonce.bytes().enumerate().all(|(i, c)| {
                if [8, 13, 18, 23].contains(&i) {
                    c == b'-'
                } else {
                    c.is_ascii_hexdigit()
                }
            });
        if !match self.scope {
            "websocket" => hex(48),
            "subscription" => hex(64),
            _ => hex(32) || uuid,
        } {
            return Err(());
        }
        if !db
            .admit_request(
                self.scope,
                client,
                timestamp,
                &nonce.to_ascii_lowercase(),
                now,
                60_000,
            )
            .map_err(|_| ())?
        {
            return Err(());
        }
        Ok(body)
    }
}
#[cfg(test)]
#[path = "../../../../tests/plain-server/unit/http/auth/replay.rs"]
mod tests;
