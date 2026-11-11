use plain_libs::anyhow::{Result, ensure};
use std::collections::HashMap;
use plain_libs::utils::uuid::Uuid;

const SESSION_CAPACITY: usize = 128;
const OWNER_CAPACITY: usize = 16;

#[derive(Clone, Default)]
pub struct DiscoverySessions {
    owners: HashMap<Uuid, String>,
}

impl DiscoverySessions {
    pub fn acquire(&mut self, owner: &str, token: Uuid) -> Result<()> {
        ensure!(!owner.is_empty(), "Missing discovery owner");
        if let Some(existing) = self.owners.get(&token) {
            ensure!(
                existing == owner,
                "Discovery session belongs to another client"
            );
            return Ok(());
        }
        ensure!(
            self.owners.len() < SESSION_CAPACITY,
            "Too many discovery sessions"
        );
        ensure!(
            self.owners
                .values()
                .filter(|existing| existing.as_str() == owner)
                .count()
                < OWNER_CAPACITY,
            "Too many discovery sessions for client"
        );
        self.owners.insert(token, owner.into());
        Ok(())
    }

    pub fn release(&mut self, owner: &str, token: Uuid) -> Result<()> {
        if let Some(existing) = self.owners.get(&token) {
            ensure!(
                existing == owner,
                "Discovery session belongs to another client"
            );
        }
        self.owners.remove(&token);
        Ok(())
    }

    pub fn release_owner(&mut self, owner: &str) -> bool {
        let previous = self.owners.len();
        self.owners.retain(|_, existing| existing != owner);
        previous != self.owners.len()
    }

    pub fn active(&self) -> bool {
        !self.owners.is_empty()
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/features/discovery/sessions.rs"]
mod tests;
