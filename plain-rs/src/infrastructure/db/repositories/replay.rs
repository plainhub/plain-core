use crate::infrastructure::db::Db;
use rusqlite::{TransactionBehavior, params};
use sha2::{Digest, Sha256};
impl Db {
    pub fn admit_request(
        &self,
        scope: &str,
        client: &str,
        timestamp: i64,
        nonce: &str,
        now: i64,
        window_ms: i64,
    ) -> rusqlite::Result<bool> {
        if client.is_empty()
            || client.len() > 256
            || nonce.is_empty()
            || nonce.len() > 256
            || window_ms < 0
        {
            return Ok(false);
        }
        let mut digest = Sha256::new();
        for part in [
            scope.as_bytes(),
            client.as_bytes(),
            &timestamp.to_be_bytes(),
            nonce.as_bytes(),
        ] {
            digest.update((part.len() as u64).to_be_bytes());
            digest.update(part);
        }
        let receipt = digest.finalize();
        self.with_conn(|conn| {
            let tx=conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let highwater:Option<i64>=tx.query_row("SELECT MAX(admitted_at) FROM replay_receipts",[],|row|row.get(0))?;
            let now=now.max(highwater.unwrap_or(now));
            if now.abs_diff(timestamp)>window_ms as u64 {return Ok(false)}
            let expires_at=timestamp.saturating_add(window_ms);
            let inserted=tx.execute("INSERT OR IGNORE INTO replay_receipts(receipt,expires_at,admitted_at) VALUES(?1,?2,?3)",params![receipt.as_slice(),expires_at,now])?;
            if inserted==0 {return Ok(false)}
            // The admitted row retains the clock watermark while indexed batches remove expired receipts.
            tx.execute("DELETE FROM replay_receipts WHERE receipt IN (SELECT receipt FROM replay_receipts WHERE expires_at<?1 ORDER BY expires_at LIMIT 256)",[now])?;
            tx.commit()?; Ok(true)
        })
    }
}
#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/infrastructure/db/repositories/replay.rs"]
mod tests;
