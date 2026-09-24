use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use tracing::{error, info, warn};

pub struct DeadmanSwitch {
    last_binance_ts: Arc<AtomicI64>,
    last_hl_ts: Arc<AtomicI64>,
    timeout_threshold_ms: i64,
    is_tripped: bool,
}

impl DeadmanSwitch {
    pub fn new(timeout_threshold_ms: i64) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            last_binance_ts: Arc::new(AtomicI64::new(now)),
            last_hl_ts: Arc::new(AtomicI64::new(now)),
            timeout_threshold_ms,
            is_tripped: false,
        }
    }

    pub fn touch_binance(&self, ts: i64) {
        self.last_binance_ts.store(ts, Ordering::Relaxed);
    }

    pub fn touch_hyperliquid(&self, ts: i64) {
        self.last_hl_ts.store(ts, Ordering::Relaxed);
    }

    pub fn check_health(&mut self, now_ms: i64) -> bool {
        let b_ts = self.last_binance_ts.load(Ordering::Relaxed);
        let hl_ts = self.last_hl_ts.load(Ordering::Relaxed);

        let b_diff = now_ms - b_ts;
        let hl_diff = now_ms - hl_ts;

        if b_diff > self.timeout_threshold_ms || hl_diff > self.timeout_threshold_ms {
            if !self.is_tripped {
                self.is_tripped = true;
                error!(
                    "🚨 [DEADMAN'S SWITCH TRIPPED] Socket disconnect/silence detected! (Binance Lag: {}ms, HL Lag: {}ms). TRIGGERING EMERGENCY CANCEL-ALL!",
                    b_diff, hl_diff
                );
            }
            return false; // Unhealthy! Emergency cancel required!
        }

        if self.is_tripped {
            self.is_tripped = false;
            info!("✅ [DEADMAN'S SWITCH RESTORED] WebSocket streams healthy again.");
        }

        true // Healthy
    }
}
