use crate::engine::broker::Broker;
use crate::engine::types::*;

pub struct OrderbookImbalanceStrategy {
    name: String,
    last_trade_time: i64,
    cooldown_ms: i64,
    trade_qty: f64,
}

impl OrderbookImbalanceStrategy {
    pub fn new(trade_qty: f64) -> Self {
        Self {
            name: "L2 Imbalance Scalper".to_string(),
            last_trade_time: 0,
            cooldown_ms: 5000, // 5s cooldown
            trade_qty,
        }
    }
}

impl super::Strategy for OrderbookImbalanceStrategy {
    fn name(&self) -> &str {
        &self.name
    }

    fn on_market_update(
        &mut self,
        market: &MarketState,
        broker: &mut dyn Broker,
    ) -> Option<StrategyDecision> {
        let now_ms = chrono::Utc::now().timestamp_millis();
        if now_ms - self.last_trade_time < self.cooldown_ms {
            return None;
        }

        // Calculate bid/ask volume imbalance from L2 top 5 levels
        let bid_vol: f64 = market.book.bids.iter().take(5).map(|l| l.qty).sum();
        let ask_vol: f64 = market.book.asks.iter().take(5).map(|l| l.qty).sum();
        let total_vol = bid_vol + ask_vol;

        if total_vol <= 0.0 || market.mark_price <= 0.0 {
            return None;
        }

        let imbalance_ratio = (bid_vol - ask_vol) / total_vol; // Range: -1.0 to 1.0
        let pos = broker.get_position();

        // Strong Bid Pressure (> +0.35) -> Buy
        if imbalance_ratio > 0.35 && pos.side != Some(Side::Buy) {
            self.last_trade_time = now_ms;
            let (_, fill_opt) = broker.submit_order(
                &market.symbol,
                Side::Buy,
                OrderType::Market,
                None,
                self.trade_qty,
                market,
            );

            let exec_px = fill_opt.map(|f| f.price).unwrap_or(market.mark_price);
            return Some(StrategyDecision {
                timestamp: now_ms,
                symbol: market.symbol.clone(),
                action: "BUY".to_string(),
                side: Some(Side::Buy),
                price: exec_px,
                qty: self.trade_qty,
                reason: format!("High Bid Pressure (Imbalance: {:.2}%)", imbalance_ratio * 100.0),
            });
        }
        // Strong Ask Pressure (< -0.35) -> Sell
        else if imbalance_ratio < -0.35 && pos.side != Some(Side::Sell) {
            self.last_trade_time = now_ms;
            let (_, fill_opt) = broker.submit_order(
                &market.symbol,
                Side::Sell,
                OrderType::Market,
                None,
                self.trade_qty,
                market,
            );

            let exec_px = fill_opt.map(|f| f.price).unwrap_or(market.mark_price);
            return Some(StrategyDecision {
                timestamp: now_ms,
                symbol: market.symbol.clone(),
                action: "SELL".to_string(),
                side: Some(Side::Sell),
                price: exec_px,
                qty: self.trade_qty,
                reason: format!("High Ask Pressure (Imbalance: {:.2}%)", imbalance_ratio * 100.0),
            });
        }

        None
    }
}
