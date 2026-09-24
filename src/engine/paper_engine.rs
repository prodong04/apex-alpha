use crate::engine::types::*;
use chrono::Utc;
use std::collections::HashMap;

pub struct PaperMatchingEngine {
    pub pending_orders: HashMap<String, Order>,
    pub order_counter: u64,
}

impl PaperMatchingEngine {
    pub fn new() -> Self {
        Self {
            pending_orders: HashMap::new(),
            order_counter: 0,
        }
    }

    /// Submit a new order to paper matching engine
    pub fn submit_order(
        &mut self,
        symbol: &str,
        side: Side,
        order_type: OrderType,
        price: Option<f64>,
        qty: f64,
        market: &MarketState,
    ) -> (Order, Option<Fill>) {
        self.order_counter += 1;
        let order_id = format!("PAPER-ORD-{}", self.order_counter);
        let now = Utc::now();
        let now_ms = now.timestamp_millis();

        let mut order = Order {
            id: order_id.clone(),
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

        let mut fill = None;

        match order_type {
            OrderType::Market => {
                // Execute immediately using L2 book depth or best bid/ask
                let exec_price = self.calculate_market_fill_price(side, qty, market);
                order.filled_qty = qty;
                order.avg_fill_price = exec_price;
                order.status = OrderStatus::Filled;
                order.updated_at = now;

                fill = Some(Fill {
                    id: format!("FILL-{}", self.order_counter),
                    trade_id: format!("TRD-{}", self.order_counter),
                    order_id: order.id.clone(),
                    symbol: order.symbol.clone(),
                    side: order.side,
                    price: exec_price,
                    qty,
                    fee: 0.0, // Calculated by PnlEngine
                    liquidity: Liquidity::Taker,
                    timestamp: now,
                    realized_pnl: 0.0,
                });
            }
            OrderType::Limit => {
                // Check if limit order is marketable immediately
                if let Some(l_px) = price {
                    let is_marketable = match side {
                        Side::Buy => market.best_ask > 0.0 && l_px >= market.best_ask,
                        Side::Sell => market.best_bid > 0.0 && l_px <= market.best_bid,
                    };

                    if is_marketable {
                        let exec_price = l_px;
                        order.filled_qty = qty;
                        order.status = OrderStatus::Filled;
                        order.updated_at = now;

                        fill = Some(Fill {
                            id: format!("FILL-{}", self.order_counter),
                            trade_id: format!("TRD-{}", self.order_counter),
                            order_id: order.id.clone(),
                            symbol: order.symbol.clone(),
                            side: order.side,
                            price: exec_price,
                            qty,
                            fee: 0.0,
                            liquidity: Liquidity::Taker,
                            timestamp: now,
                            realized_pnl: 0.0,
                        });
                    } else {
                        // Store in pending queue
                        self.pending_orders.insert(order.id.clone(), order.clone());
                    }
                }
            }
        }

        (order, fill)
    }

    /// Calculate VWAP fill price across L2 order book depth for realistic slippage
    fn calculate_market_fill_price(&self, side: Side, target_qty: f64, market: &MarketState) -> f64 {
        let levels = match side {
            Side::Buy => &market.book.asks,
            Side::Sell => &market.book.bids,
        };

        if levels.is_empty() {
            // Fallback to best bid/ask or mark price
            return match side {
                Side::Buy => if market.best_ask > 0.0 { market.best_ask } else { market.mark_price },
                Side::Sell => if market.best_bid > 0.0 { market.best_bid } else { market.mark_price },
            };
        }

        let mut remaining = target_qty;
        let mut total_cost = 0.0;

        for level in levels {
            if level.qty <= 0.0 || level.price <= 0.0 {
                continue;
            }
            let fill_at_level = remaining.min(level.qty);
            total_cost += fill_at_level * level.price;
            remaining -= fill_at_level;

            if remaining <= 1e-8 {
                break;
            }
        }

        if target_qty > 0.0 && remaining < target_qty {
            let executed_qty = target_qty - remaining;
            total_cost / executed_qty
        } else {
            match side {
                Side::Buy => market.best_ask,
                Side::Sell => market.best_bid,
            }
        }
    }

    /// Check pending limit orders against latest market tick / trade price
    pub fn check_pending_orders(&mut self, market: &MarketState) -> Vec<Fill> {
        let mut filled_ids = Vec::new();
        let mut fills = Vec::new();
        let now = Utc::now();

        for (id, order) in self.pending_orders.iter_mut() {
            if let Some(target_px) = order.price {
                let filled = match order.side {
                    Side::Buy => market.last_trade_price > 0.0 && market.last_trade_price <= target_px,
                    Side::Sell => market.last_trade_price > 0.0 && market.last_trade_price >= target_px,
                };

                if filled {
                    order.filled_qty = order.qty;
                    order.status = OrderStatus::Filled;
                    order.updated_at = now;
                    filled_ids.push(id.clone());

                    fills.push(Fill {
                        id: format!("FILL-{}", id),
                        trade_id: format!("TRD-{}", id),
                        order_id: order.id.clone(),
                        symbol: order.symbol.clone(),
                        side: order.side,
                        price: target_px,
                        qty: order.qty,
                        fee: 0.0,
                        liquidity: Liquidity::Maker, // Limit order hit is Maker
                        timestamp: now,
                        realized_pnl: 0.0,
                    });
                }
            }
        }

        for id in filled_ids {
            self.pending_orders.remove(&id);
        }

        fills
    }

    pub fn cancel_order(&mut self, order_id: &str) -> bool {
        self.pending_orders.remove(order_id).is_some()
    }
}
