use crate::engine::paper_engine::PaperMatchingEngine;
use crate::engine::pnl_engine::PnlEngine;
use crate::engine::types::*;

pub trait Broker: Send + Sync {
    fn name(&self) -> &str;
    fn is_live(&self) -> bool;
    
    fn submit_order(
        &mut self,
        symbol: &str,
        side: Side,
        order_type: OrderType,
        price: Option<f64>,
        qty: f64,
        market: &MarketState,
    ) -> (Order, Option<Fill>);

    fn cancel_order(&mut self, order_id: &str) -> bool;
    fn on_market_update(&mut self, market: &MarketState) -> Vec<Fill>;
    fn get_account(&self) -> Account;
    fn get_position(&self) -> Position;
}

pub struct PaperBroker {
    matching_engine: PaperMatchingEngine,
    pnl_engine: PnlEngine,
}

impl PaperBroker {
    pub fn new(initial_balance: f64, leverage: f64, maker_fee: f64, taker_fee: f64) -> Self {
        Self {
            matching_engine: PaperMatchingEngine::new(),
            pnl_engine: PnlEngine::new(initial_balance, leverage, maker_fee, taker_fee),
        }
    }
}

impl Broker for PaperBroker {
    fn name(&self) -> &str {
        "PaperBroker (Offline Simulation)"
    }

    fn is_live(&self) -> bool {
        false
    }

    fn submit_order(
        &mut self,
        symbol: &str,
        side: Side,
        order_type: OrderType,
        price: Option<f64>,
        qty: f64,
        market: &MarketState,
    ) -> (Order, Option<Fill>) {
        let (order, mut fill_opt) = self.matching_engine.submit_order(symbol, side, order_type, price, qty, market);
        if let Some(ref mut fill) = fill_opt {
            self.pnl_engine.on_fill(fill);
        }
        (order, fill_opt)
    }

    fn cancel_order(&mut self, order_id: &str) -> bool {
        self.matching_engine.cancel_order(order_id)
    }

    fn on_market_update(&mut self, market: &MarketState) -> Vec<Fill> {
        let mut fills = self.matching_engine.check_pending_orders(market);
        for fill in fills.iter_mut() {
            self.pnl_engine.on_fill(fill);
        }
        self.pnl_engine.update_market(market.mark_price);
        fills
    }

    fn get_account(&self) -> Account {
        self.pnl_engine.account.clone()
    }

    fn get_position(&self) -> Position {
        self.pnl_engine.position.clone()
    }
}

pub struct LiveBroker {
    pnl_engine: PnlEngine,
}

impl LiveBroker {
    pub fn new(initial_balance: f64, leverage: f64, maker_fee: f64, taker_fee: f64) -> Self {
        Self {
            pnl_engine: PnlEngine::new(initial_balance, leverage, maker_fee, taker_fee),
        }
    }
}

impl Broker for LiveBroker {
    fn name(&self) -> &str {
        "LiveBroker (Hyperliquid / Binance API)"
    }

    fn is_live(&self) -> bool {
        true
    }

    fn submit_order(
        &mut self,
        symbol: &str,
        side: Side,
        order_type: OrderType,
        price: Option<f64>,
        qty: f64,
        _market: &MarketState,
    ) -> (Order, Option<Fill>) {
        // Live order execution bridge
        // Will sign and send request to Hyperliquid / Binance API once credentials provided
        tracing::warn!("⚡ [LIVE BROKER] Submitting live order: {:?} {} {} @ {:?}", side, qty, symbol, price);
        
        let now = chrono::Utc::now();
        let now_ms = now.timestamp_millis();
        let order = Order {
            id: format!("LIVE-ORD-{}", now_ms),
            symbol: symbol.to_string(),
            side,
            order_type,
            price,
            qty,
            filled_qty: 0.0,
            avg_fill_price: 0.0,
            status: OrderStatus::New,
            timestamp: now_ms,
            created_at: now,
            updated_at: now,
        };
        (order, None)
    }

    fn cancel_order(&mut self, _order_id: &str) -> bool {
        true
    }

    fn on_market_update(&mut self, market: &MarketState) -> Vec<Fill> {
        self.pnl_engine.update_market(market.mark_price);
        Vec::new()
    }

    fn get_account(&self) -> Account {
        self.pnl_engine.account.clone()
    }

    fn get_position(&self) -> Position {
        self.pnl_engine.position.clone()
    }
}
