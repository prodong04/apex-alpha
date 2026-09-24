use crate::engine::types::UiEvent;
use tokio::sync::broadcast;

#[derive(Clone)]
pub struct UiBroadcaster {
    tx: broadcast::Sender<UiEvent>,
}

impl UiBroadcaster {
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }

    /// Non-blocking event emission into broadcast channel (Zero-impact on hot path)
    #[inline(always)]
    pub fn send(&self, event: UiEvent) {
        // Send will fail silently if no receivers are active, incurring near-zero CPU overhead.
        let _ = self.tx.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<UiEvent> {
        self.tx.subscribe()
    }
}
