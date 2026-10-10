use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    sync::{Arc, RwLock},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub unix_millis: u128,
    pub level: String,
    pub message: String,
}

#[derive(Clone)]
pub struct LogRing {
    inner: Arc<RwLock<VecDeque<LogEntry>>>,
    capacity: usize,
}

impl LogRing {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Arc::new(RwLock::new(VecDeque::with_capacity(capacity.max(1)))),
            capacity: capacity.max(1),
        }
    }

    pub fn push(&self, level: impl Into<String>, message: impl Into<String>) {
        let mut entries = self.inner.write().expect("log lock poisoned");
        if entries.len() == self.capacity {
            entries.pop_front();
        }
        entries.push_back(LogEntry {
            unix_millis: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
            level: level.into(),
            message: message.into(),
        });
    }

    pub fn snapshot(&self) -> Vec<LogEntry> {
        self.inner
            .read()
            .expect("log lock poisoned")
            .iter()
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_keeps_newest_entries() {
        let ring = LogRing::new(2);
        ring.push("info", "one");
        ring.push("info", "two");
        ring.push("info", "three");
        let entries = ring.snapshot();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].message, "two");
        assert_eq!(entries[1].message, "three");
    }
}
