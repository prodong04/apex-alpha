pub mod binance_futures;
pub mod binance_spot;
pub mod hyperliquid;
pub mod types;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Clone, Default)]
pub struct ExchangePricesHolder {
    pub spot_px_bits: Arc<AtomicU64>,
    pub fut_px_bits: Arc<AtomicU64>,
    pub hl_px_bits: Arc<AtomicU64>,
}

impl ExchangePricesHolder {
    pub fn new() -> Self {
        Self::default()
    }

    #[inline(always)]
    pub fn set_spot(&self, px: f64) {
        if px > 0.0 {
            self.spot_px_bits.store(px.to_bits(), Ordering::Relaxed);
        }
    }

    #[inline(always)]
    pub fn set_fut(&self, px: f64) {
        if px > 0.0 {
            self.fut_px_bits.store(px.to_bits(), Ordering::Relaxed);
        }
    }

    #[inline(always)]
    pub fn set_hl(&self, px: f64) {
        if px > 0.0 {
            self.hl_px_bits.store(px.to_bits(), Ordering::Relaxed);
        }
    }

    #[inline(always)]
    pub fn get_prices(&self) -> (f64, f64, f64) {
        (
            f64::from_bits(self.spot_px_bits.load(Ordering::Relaxed)),
            f64::from_bits(self.fut_px_bits.load(Ordering::Relaxed)),
            f64::from_bits(self.hl_px_bits.load(Ordering::Relaxed)),
        )
    }
}
