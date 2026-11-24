//! Persisted queue of register writes waiting for upload (PROTOCOL.md §7.3).
//!
//! The same structure backs `outbox.json` (personal collections) and
//! `shared_outbox.json` (shared playlists: `collection` = playlist id, `key` = entry id).
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{path::Path, time::Duration};
use uuid::Uuid;

/// Hard cap; beyond it the oldest `history` writes are dropped first.
pub const MAX_OPS: usize = 20_000;
/// Operations per `POST /api/sync/push`.
pub const PUSH_BATCH: usize = 500;
pub const MAX_BACKOFF: Duration = Duration::from_secs(300);
pub const RATE_LIMIT_BACKOFF: Duration = Duration::from_secs(60);
pub const FULL_MESSAGE: &str = "Too many unsynced changes; connect to the server to sync them";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OutboxOp {
    pub id: Uuid,
    pub collection: String,
    pub key: String,
    pub value: Option<Value>,
    pub clock: i64,
    #[serde(default)]
    pub attempts: u32,
    #[serde(default)]
    pub next_at: i64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Outbox {
    #[serde(default)]
    pub ops: Vec<OutboxOp>,
}

/// `min(2^attempts × 2 s, 5 min)` before jitter.
pub fn backoff(attempts: u32) -> Duration {
    Duration::from_secs(2)
        .saturating_mul(1u32.checked_shl(attempts.min(16)).unwrap_or(u32::MAX))
        .min(MAX_BACKOFF)
}

impl Outbox {
    pub fn load(path: &Path) -> Result<Self> {
        Ok(crate::store::read_json(path)?.unwrap_or_default())
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        crate::store::write_json(path, self)
    }
    pub fn len(&self) -> usize {
        self.ops.len()
    }
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// Queues a write, replacing any pending write of the same register.
    pub fn enqueue(
        &mut self,
        collection: &str,
        key: &str,
        value: Option<Value>,
        clock: i64,
    ) -> Result<()> {
        self.ops
            .retain(|op| !(op.collection == collection && op.key == key));
        if self.ops.len() >= MAX_OPS {
            if let Some(i) = self.ops.iter().position(|op| op.collection == "history") {
                self.ops.remove(i);
            } else {
                bail!(FULL_MESSAGE);
            }
        }
        self.ops.push(OutboxOp {
            id: Uuid::new_v4(),
            collection: collection.into(),
            key: key.into(),
            value,
            clock,
            attempts: 0,
            next_at: 0,
        });
        Ok(())
    }

    /// The pending write of a register, if any.
    pub fn pending(&self, collection: &str, key: &str) -> Option<&OutboxOp> {
        self.ops
            .iter()
            .find(|op| op.collection == collection && op.key == key)
    }

    /// Up to `limit` operations whose retry time has come, oldest first.
    pub fn due(&self, now: i64, limit: usize) -> Vec<OutboxOp> {
        self.ops
            .iter()
            .filter(|op| op.next_at <= now)
            .take(limit)
            .cloned()
            .collect()
    }

    /// The earliest retry time of a deferred operation.
    pub fn next_retry(&self) -> Option<i64> {
        self.ops
            .iter()
            .map(|op| op.next_at)
            .filter(|t| *t > 0)
            .min()
    }

    /// Removes finished operations. A register rewritten meanwhile has a new op id and stays.
    pub fn complete(&mut self, ids: &[Uuid]) {
        self.ops.retain(|op| !ids.contains(&op.id));
    }

    /// Backs off after a failed upload: exponential per op, or a fixed delay (429).
    pub fn defer(&mut self, ids: &[Uuid], now: i64, fixed: Option<Duration>) {
        for op in self.ops.iter_mut().filter(|op| ids.contains(&op.id)) {
            op.attempts = op.attempts.saturating_add(1);
            let wait = fixed.unwrap_or_else(|| crate::jitter(backoff(op.attempts - 1)));
            op.next_at = now + wait.as_millis() as i64;
        }
    }

    /// Makes every op due now (after reconnecting or signing in again).
    pub fn reset_backoff(&mut self) {
        for op in &mut self.ops {
            op.attempts = 0;
            op.next_at = 0;
        }
    }

    pub fn remove_collection(&mut self, collection: &str) {
        self.ops.retain(|op| op.collection != collection);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn enqueue_coalesces_per_register() {
        let mut outbox = Outbox::default();
        outbox
            .enqueue("saved", "youtube:a", Some(json!(1)), 10)
            .unwrap();
        let first = outbox.ops[0].id;
        outbox
            .enqueue("saved", "youtube:b", Some(json!(2)), 11)
            .unwrap();
        outbox.enqueue("saved", "youtube:a", None, 12).unwrap();
        assert_eq!(outbox.len(), 2);
        let a = outbox.pending("saved", "youtube:a").unwrap();
        assert_ne!(a.id, first);
        assert_eq!((a.value.clone(), a.clock), (None, 12));
        // Completing the replaced op id leaves the newer write queued.
        outbox.complete(&[first]);
        assert_eq!(outbox.len(), 2);
    }

    #[test]
    fn cap_drops_oldest_history_then_refuses() {
        let mut outbox = Outbox::default();
        outbox.enqueue("history", "h0", Some(json!(0)), 1).unwrap();
        for i in 1..MAX_OPS {
            outbox
                .enqueue("saved", &format!("k{i}"), Some(json!(i)), 1)
                .unwrap();
        }
        assert_eq!(outbox.len(), MAX_OPS);
        outbox.enqueue("saved", "extra", None, 1).unwrap();
        assert_eq!(outbox.len(), MAX_OPS);
        assert!(outbox.pending("history", "h0").is_none());
        let error = outbox.enqueue("saved", "more", None, 1).unwrap_err();
        assert_eq!(error.to_string(), FULL_MESSAGE);
        // Rewriting an existing register still works when full.
        outbox.enqueue("saved", "k5", None, 2).unwrap();
    }

    #[test]
    fn backoff_doubles_to_five_minutes_with_jitter() {
        assert_eq!(backoff(0), Duration::from_secs(2));
        assert_eq!(backoff(1), Duration::from_secs(4));
        assert_eq!(backoff(6), Duration::from_secs(128));
        assert_eq!(backoff(7), Duration::from_secs(256));
        assert_eq!(backoff(8), MAX_BACKOFF);
        assert_eq!(backoff(100), MAX_BACKOFF);
        let mut outbox = Outbox::default();
        outbox.enqueue("saved", "a", None, 1).unwrap();
        let id = outbox.ops[0].id;
        outbox.defer(&[id], 1_000, None);
        let wait = outbox.ops[0].next_at - 1_000;
        assert!((1600..=2400).contains(&wait), "{wait}");
        assert!(outbox.due(1_000, 10).is_empty());
        assert_eq!(outbox.due(1_000 + wait, 10).len(), 1);
        outbox.defer(&[id], 1_000, Some(RATE_LIMIT_BACKOFF));
        assert_eq!(outbox.ops[0].next_at, 61_000);
        assert_eq!(outbox.ops[0].attempts, 2);
        assert_eq!(outbox.next_retry(), Some(61_000));
        outbox.reset_backoff();
        assert_eq!(outbox.due(0, 10).len(), 1);
    }

    #[test]
    fn persists_as_the_contract_shape() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("outbox.json");
        let mut outbox = Outbox::default();
        outbox
            .enqueue("filters", "f", Some(json!({"kind":"live"})), 5)
            .unwrap();
        outbox.save(&path).unwrap();
        let raw: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let op = &raw["ops"][0];
        for field in [
            "id",
            "collection",
            "key",
            "value",
            "clock",
            "attempts",
            "next_at",
        ] {
            assert!(op.get(field).is_some(), "{field}");
        }
        assert_eq!(Outbox::load(&path).unwrap().ops, outbox.ops);
    }
}
