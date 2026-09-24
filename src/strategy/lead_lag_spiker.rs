use crate::engine::broker::Broker;
use crate::engine::types::*;
use std::collections::VecDeque;

pub struct LeadLagSpikerStrategy {
    name: String,
    spike_threshold: f64,
    lookback_ms: i64,
    timeout_ms: i64,
    size_btc: f64,
    max_inventory_btc: f64,
    binance_window: VecDeque<(i64, f64)>,
    cooldown_until: i64,
}

impl LeadLagSpikerStrategy {
    pub fn new(spike_threshold: f64, size_btc: f64, timeout_ms: i64) -> Self {
        Self {
            name: "Lead-Lag 1s Delta Shock Sniping (Baseline: 70% Convergence / 500ms Fast Cancel)".to_string(),
            spike_threshold,
            lookback_ms: 1000, // 1 second rolling window
            timeout_ms,
            size_btc,
            max_inventory_btc: 1.0,
            binance_window: VecDeque::with_capacity(500),
            cooldown_until: 0,
        }
    }

    pub fn on_binance_spot_tick(&mut self, price: f64, now_ms: i64) {
        self.binance_window.push_back((now_ms, price));
        while let Some(&(old_ts, _)) = self.binance_window.front() {
            if now_ms - old_ts > self.lookback_ms {
                self.binance_window.pop_front();
            } else {
                break;
            }
        }
    }

    pub fn check_signal(
        &mut self,
        hl_best_bid: f64,
        hl_best_ask: f64,
        current_position: &Position,
        now_ms: i64,
    ) -> Option<StrategyDecision> {
        if now_ms < self.cooldown_until || self.binance_window.len() < 2 {
            return None;
        }

        let oldest_px = self.binance_window.front()?.1;
        let newest_px = self.binance_window.back()?.1;
        let delta = newest_px - oldest_px;

        let current_pos_size = current_position.size;
        let pos_side = current_position.side;

        // 1. Long Signal (Binance Spot surged by +$80~100 in 1s)
        if delta >= self.spike_threshold {
            if pos_side == Some(Side::Buy) && current_pos_size >= self.max_inventory_btc {
                return None;
            }

            self.cooldown_until = now_ms + 1500;
            let limit_px = if hl_best_bid > 0.0 { hl_best_bid + 0.5 } else { newest_px };
            let target_px = limit_px + (delta * 0.7); // 70% gap convergence

            return Some(StrategyDecision {
                timestamp: now_ms,
                symbol: "BTC".to_string(),
                action: "POST_ONLY_BUY".to_string(),
                side: Some(Side::Buy),
                price: limit_px,
                qty: self.size_btc,
                reason: format!("⚡ Binance Spot 1s Surge (+${:.1}) ➔ ALO BestBid+0.5 (Target ${:.1})", delta, target_px),
            });
        }

        // 2. Short Signal (Binance Spot dumped by -$80~100 in 1s)
        if delta <= -self.spike_threshold {
            if pos_side == Some(Side::Sell) && current_pos_size >= self.max_inventory_btc {
                return None;
            }

            self.cooldown_until = now_ms + 1500;
            let limit_px = if hl_best_ask > 0.0 { hl_best_ask - 0.5 } else { newest_px };
            let target_px = limit_px - (delta.abs() * 0.7);

            return Some(StrategyDecision {
                timestamp: now_ms,
                symbol: "BTC".to_string(),
                action: "POST_ONLY_SELL".to_string(),
                side: Some(Side::Sell),
                price: limit_px,
                qty: self.size_btc,
                reason: format!("⚡ Binance Spot 1s Dump (-${:.1}) ➔ ALO BestAsk-0.5 (Target ${:.1})", delta.abs(), target_px),
            });
        }

        None
    }
}

impl super::Strategy for LeadLagSpikerStrategy {
    fn name(&self) -> &str {
        &self.name
    }

    fn on_market_update(
        &mut self,
        _market: &MarketState,
        _broker: &mut dyn Broker,
    ) -> Option<StrategyDecision> {
        None
    }
}
