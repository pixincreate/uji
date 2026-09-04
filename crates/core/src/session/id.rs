use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Monotonic millisecond counter — mirrors opencode's `ascending()` id scheme:
/// time-ordered, but never collides within the same millisecond.
fn ascending() -> i64 {
    static LAST: AtomicU64 = AtomicU64::new(0);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let mut prev = LAST.load(Ordering::Relaxed) as i64;
    loop {
        let next = now.max(prev + 1);
        match LAST.compare_exchange_weak(prev as u64, next as u64, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return next,
            Err(cur) => prev = cur as i64,
        }
    }
}

pub fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

macro_rules! id_type {
    ($name:ident, $prefix:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(pub String);

        impl $name {
            pub fn new() -> Self {
                Self(format!(concat!($prefix, "{}"), ascending()))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }
    };
}

id_type!(SessionId, "ses_");
id_type!(MessageId, "msg_");
