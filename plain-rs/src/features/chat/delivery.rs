use crate::features::chat::channels::chat_helper::ChannelDeliveryResult;
use crate::features::chat::channels::messages::ChannelMember;
use crate::features::chat::enums::ChannelStatus;
use crate::infrastructure::db::DChat;
use crate::infrastructure::db::Db;
use crate::infrastructure::db::chat_store::channels;
use crate::infrastructure::db::chat_store::messages;
use crate::infrastructure::db::chat_store::peers;
use crate::infrastructure::transport::contract::PeerTransport;
use crate::infrastructure::transport::contract::chat_item_request;
use plain_libs::anyhow::{Result, bail};
use std::{
    collections::{HashMap, HashSet},
    future::{Future, poll_fn},
    sync::{Arc, Mutex, Weak},
    task::Poll,
};

#[derive(serde::Serialize)]
pub struct Receipt {
    pub chat: Option<DChat>,
    pub rediscover: bool,
}

type Locks = Mutex<HashMap<String, Weak<tokio::sync::Mutex<()>>>>;

pub struct Delivery {
    db: Db,
    resource_token: Arc<str>,
    locks: Locks,
    peer_locks: Locks,
}
impl Delivery {
    pub fn new(db: Db, resource_token: Arc<str>) -> Self {
        Self {
            db,
            resource_token,
            locks: Locks::default(),
            peer_locks: Locks::default(),
        }
    }
    fn lock_for(locks: &Locks, id: &str) -> Arc<tokio::sync::Mutex<()>> {
        let mut locks = locks.lock().unwrap();
        locks.retain(|_, lock| lock.strong_count() != 0);
        if let Some(lock) = locks.get(id).and_then(Weak::upgrade) {
            return lock;
        }
        let lock = Arc::new(tokio::sync::Mutex::new(()));
        locks.insert(id.into(), Arc::downgrade(&lock));
        lock
    }
    pub async fn send<T: PeerTransport>(
        &self,
        transport: &T,
        client_id: &str,
        signing_key: &[u8],
        id: &str,
        recipients: Option<Vec<String>>,
    ) -> Result<Receipt> {
        self.send_observed(transport, client_id, signing_key, id, recipients, |_| {})
            .await
    }
    pub async fn send_observed<T: PeerTransport>(
        &self,
        transport: &T,
        client_id: &str,
        signing_key: &[u8],
        id: &str,
        recipients: Option<Vec<String>>,
        pending: impl Fn(&DChat) + Send,
    ) -> Result<Receipt> {
        let lock = Self::lock_for(&self.locks, id);
        let _guard = lock.lock().await;
        let Some(chat) = messages::get(&self.db, id)? else {
            return Ok(Receipt {
                chat: None,
                rediscover: false,
            });
        };
        if chat.from_id != "me" {
            bail!("Only outgoing messages can be sent");
        }
        let retry = recipients.is_some();
        if chat.channel_id.is_none() && (chat.to_id.is_empty() || chat.to_id == "local") {
            if retry {
                bail!("Targeted retry requires a channel");
            }
            return Ok(Receipt {
                chat: Some(chat),
                rediscover: false,
            });
        }
        let content = crate::features::chat::messages::content::peer_content(
            &chat.content,
            client_id,
            &self.resource_token,
        )?;
        let targets = if chat.channel_id.is_none() {
            if retry {
                bail!("Targeted retry requires a channel");
            }
            vec![chat.to_id.clone()]
        } else {
            let Some(channel) =
                channels::get(&self.db, chat.channel_id.as_deref().unwrap_or_default())?
            else {
                return self.finish(&chat, None, true);
            };
            let members: Vec<ChannelMember> = plain_libs::serde_json::from_str(&channel.members)?;
            if channel.status != ChannelStatus::Joined
                || !members
                    .iter()
                    .any(|m| m.peer_id == client_id && m.is_joined())
            {
                return self.finish(&chat, None, true);
            }
            let joined: HashSet<_> = members
                .iter()
                .filter(|m| m.is_joined() && m.peer_id != client_id)
                .map(|m| m.peer_id.clone())
                .collect();
            match recipients {
                Some(ids) => {
                    let mut unique = HashSet::new();
                    if ids
                        .iter()
                        .any(|id| !joined.contains(id) || !unique.insert(id))
                    {
                        bail!("Invalid retry recipients");
                    }
                    ids
                }
                None => members
                    .into_iter()
                    .filter(|m| m.is_joined() && m.peer_id != client_id)
                    .map(|m| m.peer_id)
                    .collect(),
            }
        };
        let prior = if retry {
            crate::features::chat::messages::message_lifecycle::previous_delivery_results(
                &chat, &targets,
            )?
        } else {
            Vec::new()
        };
        let chat = crate::features::chat::messages::message_lifecycle::write_delivery(
            &self.db,
            &chat,
            Some(prior.clone()),
            true,
        )?
        .ok_or_else(|| plain_libs::anyhow::anyhow!("Message changed before delivery"))?;
        pending(&chat);
        let mut jobs: Vec<_> = targets
            .iter()
            .enumerate()
            .map(|(index, target)| {
                (
                    index,
                    Box::pin(self.deliver_to(
                        transport,
                        client_id,
                        signing_key,
                        &chat,
                        &content,
                        target,
                    )),
                )
            })
            .collect();
        let mut results = vec![None; targets.len()];
        let mut rediscover = false;
        while !jobs.is_empty() {
            let (slot, result) = poll_fn(|cx| {
                for (slot, (_, job)) in jobs.iter_mut().enumerate() {
                    if let Poll::Ready(result) = job.as_mut().poll(cx) {
                        return Poll::Ready((slot, result));
                    }
                }
                Poll::Pending
            })
            .await;
            let (index, _) = jobs.remove(slot);
            let result = result?;
            rediscover |= result.error.is_some();
            results[index] = Some(result);
            if !jobs.is_empty() {
                let updated = crate::features::chat::messages::message_lifecycle::write_delivery(
                    &self.db,
                    &chat,
                    Some(
                        prior
                            .iter()
                            .cloned()
                            .chain(results.iter().flatten().cloned())
                            .collect(),
                    ),
                    true,
                )?
                .ok_or_else(|| plain_libs::anyhow::anyhow!("Message removed"))?;
                pending(&updated);
            }
        }
        drop(jobs);
        self.finish(
            &chat,
            Some(
                prior
                    .into_iter()
                    .chain(results.into_iter().flatten())
                    .collect(),
            ),
            rediscover,
        )
    }

    async fn deliver_to<T: PeerTransport>(
        &self,
        transport: &T,
        client_id: &str,
        signing_key: &[u8],
        chat: &DChat,
        content: &str,
        target: &str,
    ) -> Result<ChannelDeliveryResult> {
        let lock = Self::lock_for(&self.peer_locks, target);
        let _guard = lock.lock().await;
        let peer = peers::get(&self.db, target)?;
        let name = peer
            .as_ref()
            .map(|p| p.name.clone())
            .unwrap_or_else(|| target.into());
        let result = |error| ChannelDeliveryResult {
            peer_id: target.into(),
            peer_name: name,
            error,
        };
        let Some(peer) = peer else {
            return Ok(result(Some("Peer not found in database".into())));
        };
        let cid = chat.channel_id.as_deref().unwrap_or_default();
        let channel_key;
        let key = if cid.is_empty() {
            if !peer.is_paired() {
                return Ok(result(Some("peer unpaired".into())));
            }
            &peer.key
        } else {
            channel_key = self.channel_key_for(cid, client_id, target)?;
            &channel_key
        };
        let key_bytes = plain_libs::utils::base64::base64_decode(key);
        if key_bytes.len() != 32 {
            return Ok(result(Some("Invalid peer transport key".into())));
        }
        let signed = std::sync::OnceLock::<(String, tokio::time::Instant)>::new();
        let error = transport
            .queued_message(&peer, client_id, cid, &key_bytes, || {
                if !cid.is_empty()
                    && self
                        .channel_key_for(cid, client_id, target)
                        .map_err(|e| e.to_string())?
                        != *key
                {
                    return Err("Channel key changed during delivery".into());
                }
                if let Some((request, issued)) = signed.get() {
                    if issued.elapsed().as_millis()
                        >= u128::from(crate::infrastructure::transport::auth::GRAPHQL_WINDOW_MS)
                    {
                        return Err(
                            "Peer delivery unconfirmed: request expired while queued".into()
                        );
                    }
                    return Ok(request.clone());
                }
                let request = chat_item_request(signing_key, content, cid)?;
                let _ = signed.set((request.clone(), tokio::time::Instant::now()));
                Ok(request)
            })
            .await
            .err();
        Ok(result(error))
    }

    fn channel_key_for(&self, cid: &str, client_id: &str, target: &str) -> Result<String> {
        let channel = channels::get(&self.db, cid)?
            .ok_or_else(|| plain_libs::anyhow::anyhow!("Channel unavailable during delivery"))?;
        let members: Vec<ChannelMember> = plain_libs::serde_json::from_str(&channel.members)?;
        if channel.status != ChannelStatus::Joined
            || !members
                .iter()
                .any(|m| m.peer_id == client_id && m.is_joined())
            || !members.iter().any(|m| m.peer_id == target && m.is_joined())
        {
            bail!("Channel membership changed during delivery");
        }
        Ok(channel.key)
    }
    fn finish(
        &self,
        chat: &DChat,
        results: Option<Vec<ChannelDeliveryResult>>,
        rediscover: bool,
    ) -> Result<Receipt> {
        Ok(Receipt {
            chat: crate::features::chat::messages::message_lifecycle::write_delivery(
                &self.db, chat, results, false,
            )?,
            rediscover,
        })
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/features/chat/delivery.rs"]
mod tests;

mod receiver;

mod sender;
