use codesh_protocol::{SessionId, SessionStatus};

use super::Catalog;

pub(crate) const RETENTION_SECONDS: u64 = 7 * 24 * 60 * 60;

impl Catalog {
    pub(crate) fn expired_sessions(&self, now: u64) -> Vec<SessionId> {
        self.retry_exits();
        self.lock()
            .sessions
            .values()
            .filter(|entry| {
                matches!(
                    entry.status,
                    SessionStatus::Ended | SessionStatus::Unavailable
                ) && entry
                    .ended_at
                    .is_some_and(|ended| now.saturating_sub(ended) >= RETENTION_SECONDS)
            })
            .map(|entry| entry.info.id)
            .collect()
    }
}
