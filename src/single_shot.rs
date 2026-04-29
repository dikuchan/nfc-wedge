use std::time::{Duration, Instant};

/// Guards against duplicate NFC reads within a cooldown period.
/// Tracks the last seen UID and timestamp to prevent spam.
pub struct CooldownGuard {
    last_uid: Option<Vec<u8>>,
    last_time: Option<Instant>,
    cooldown: Duration,
}

impl CooldownGuard {
    /// Creates a new guard with the specified cooldown duration.
    pub fn new(cooldown: Duration) -> Self {
        Self {
            last_uid: None,
            last_time: None,
            cooldown,
        }
    }

    /// Checks if the given UID should be processed.
    /// Returns true if:
    /// - This is the first read, or
    /// - UID is different from last read, or
    /// - Cooldown period has elapsed since last read
    pub fn should_process(&mut self, uid: &[u8]) -> bool {
        let now = Instant::now();

        let allow = match (&self.last_uid, self.last_time) {
            (Some(last), Some(time)) => {
                // Different UID or cooldown expired
                last != uid || now.duration_since(time) >= self.cooldown
            }
            _ => true, // First read always allowed
        };

        if allow {
            self.last_uid = Some(uid.to_vec());
            self.last_time = Some(now);
        }

        allow
    }

    /// Updates cooldown duration.
    pub fn set_cooldown(&mut self, cooldown: Duration) {
        self.cooldown = cooldown;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn first_read_allowed() {
        let mut guard = CooldownGuard::new(Duration::from_secs(2));
        assert!(guard.should_process(b"uid1"));
    }

    #[test]
    fn same_uid_within_cooldown_blocked() {
        let mut guard = CooldownGuard::new(Duration::from_millis(100));
        assert!(guard.should_process(b"uid1"));
        assert!(!guard.should_process(b"uid1"));
    }

    #[test]
    fn different_uid_allowed() {
        let mut guard = CooldownGuard::new(Duration::from_secs(2));
        assert!(guard.should_process(b"uid1"));
        assert!(guard.should_process(b"uid2"));
    }

    #[test]
    fn same_uid_after_cooldown_allowed() {
        let mut guard = CooldownGuard::new(Duration::from_millis(50));
        assert!(guard.should_process(b"uid1"));
        thread::sleep(Duration::from_millis(60));
        assert!(guard.should_process(b"uid1"));
    }
}
