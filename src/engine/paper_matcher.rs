use crate::engine::types::*;
use std::collections::HashMap;
use tracing::{info, warn};

pub struct PaperMatcher {
    initial_balance: f64,
    balance: f64,
    realized_pnl: f64,
    total_fees: f64,
    maker_fee_rate: f64,
    active_orders: HashMap<u64, LimitOrder>,
    position: Position,
    position_entry_ts: i64,
    target_price: f64,
    order_id_counter: u64,
    total_trades_count: u64,
    winning_trades_count: u64,
    stop_loss_usd: f64,
    max_position_hold_ms: i64,
}

impl PaperMatcher {
    pub fn new(initial_balance: f64, maker_fee_rate: f64) -> Self {
        Self {
            initial_balance,
            balance: initial_balance,
            realized_pnl: 0.0,
            total_fees: 0.0,
            maker_fee_rate,
            active_orders: HashMap::new(),
            position: Position::default(),
            position_entry_ts: 0,
            target_price: 0.0,
            order_id_counter: 0,
            total_trades_count: 0,
            winning_trades_count: 0,
            stop_loss_usd: 40.0,        // Baseline -$40 stop loss
            max_position_hold_ms: 3000,  // Baseline 3.0s timeout
        }
    }

    /// Place a new Post-Only Limit Order with 500ms Fast Cancel timeout
    pub fn place_order(
        &mut self,
        symbol: &str,
        side: Side,
        price: f64,
        qty: f64,
        timeout_ms: i64,
        now_ms: i64,
        reason: &str,
    ) -> LimitOrder {
        self.order_id_counter += 1;
        let order = LimitOrder {
            order_id: self.order_id_counter,
            symbol: symbol.to_string(),
            side,
            price,
            qty,
            filled_qty: 0.0,
            placed_ts: now_ms,
            timeout_ms,
            status: OrderStatus::Pending,
            reason: reason.to_string(),
        };

        self.active_orders.insert(order.order_id, order.clone());
        info!(
            "📋 [ORDER PLACED] #{} | {:?} {:.4} BTC @ ${:.2} | Fast Cancel in {}ms | Reason: {}",
            order.order_id, order.side, order.qty, order.price, order.timeout_ms, order.reason
        );
        order
    }

    /// Match orders against REAL Hyperliquid Market Trade Ticks
    pub fn on_real_market_trade(
        &mut self,
        trade_px: f64,
        _trade_qty: f64,
        now_ms: i64,
    ) -> Vec<OrderFill> {
        let mut fills = Vec::new();
        let mut filled_ids = Vec::new();

        for (id, order) in self.active_orders.iter_mut() {
            let mut is_filled = false;

            match order.side {
                Side::Buy => {
                    if trade_px <= order.price {
                        is_filled = true;
                    }
                }
                Side::Sell => {
                    if trade_px >= order.price {
                        is_filled = true;
                    }
                }
            }

            if is_filled {
                let fill_px = order.price;
                let fill_qty = order.qty;
                let fee = fill_px * fill_qty * self.maker_fee_rate;
                let mut realized_pnl = 0.0;

                // Update Position & PnL
                if let Some(pos_side) = self.position.side {
                    if pos_side != order.side {
                        let pnl = if pos_side == Side::Buy {
                            (fill_px - self.position.entry_price) * fill_qty
                        } else {
                            (self.position.entry_price - fill_px) * fill_qty
                        };
                        realized_pnl = pnl;
                        self.realized_pnl += pnl;

                        self.total_trades_count += 1;
                        if pnl > 0.0 {
                            self.winning_trades_count += 1;
                        }

                        self.position.size = (self.position.size - fill_qty).max(0.0);
                        if self.position.size <= 0.00001 {
                            self.position.side = None;
                            self.position.entry_price = 0.0;
                            self.position_entry_ts = 0;
                            self.target_price = 0.0;
                        }
                    } else {
                        let total_size = self.position.size + fill_qty;
                        self.position.entry_price =
                            ((self.position.entry_price * self.position.size) + (fill_px * fill_qty)) / total_size;
                        self.position.size = total_size;
                    }
                } else {
                    self.position.side = Some(order.side);
                    self.position.size = fill_qty;
                    self.position.entry_price = fill_px;
                    self.position_entry_ts = now_ms;
                    // Target: 70% gap convergence ($50~70 target)
                    self.target_price = if order.side == Side::Buy { fill_px + 60.0 } else { fill_px - 60.0 };
                }

                self.total_fees += fee;
                self.balance += realized_pnl - fee;

                fills.push(OrderFill {
                    timestamp: now_ms,
                    order_id: *id,
                    symbol: order.symbol.clone(),
                    side: order.side,
                    price: fill_px,
                    qty: fill_qty,
                    fee,
                    realized_pnl,
                    is_maker: true,
                });

                filled_ids.push(*id);
                info!(
                    "🎯 [ORDER FILLED (MAKER)] #{} | {:?} {:.4} BTC @ ${:.2} | Fee: -${:.4} | Realized PnL: ${:+.2}",
                    id, order.side, fill_qty, fill_px, fee, realized_pnl
                );
            }
        }

        for id in filled_ids {
            self.active_orders.remove(&id);
        }

        fills
    }

    /// Check Baseline Position Exit (70% Target Hit, -$40 Stop Loss, or 3.0s Timeout)
    pub fn check_position_exit(&mut self, current_price: f64, now_ms: i64) -> Option<OrderFill> {
        let pos_side = self.position.side?;
        if self.position.size <= 0.00001 || current_price <= 0.0 {
            return None;
        }

        let entry_px = self.position.entry_price;
        let size = self.position.size;
        let hold_time = now_ms - self.position_entry_ts;

        let mut should_exit = false;
        let mut exit_reason = "3.0s Timeout Exit";

        match pos_side {
            Side::Buy => {
                if current_price >= self.target_price && self.target_price > 0.0 {
                    should_exit = true;
                    exit_reason = "🎯 70% Gap Target Hit";
                } else if current_price <= entry_px - self.stop_loss_usd {
                    should_exit = true;
                    exit_reason = "🛑 Stop Loss (-$40)";
                } else if hold_time >= self.max_position_hold_ms {
                    should_exit = true;
                    exit_reason = "⏰ 3.0s Hold Timeout Expired";
                }
            }
            Side::Sell => {
                if current_price <= self.target_price && self.target_price > 0.0 {
                    should_exit = true;
                    exit_reason = "🎯 70% Gap Target Hit";
                } else if current_price >= entry_px + self.stop_loss_usd {
                    should_exit = true;
                    exit_reason = "🛑 Stop Loss (-$40)";
                } else if hold_time >= self.max_position_hold_ms {
                    should_exit = true;
                    exit_reason = "⏰ 3.0s Hold Timeout Expired";
                }
            }
        }

        if should_exit {
            self.order_id_counter += 1;
            let exit_order_id = self.order_id_counter;
            let exit_side = pos_side.opposite();
            let exit_px = current_price;
            let fee = exit_px * size * self.maker_fee_rate;

            let pnl = if pos_side == Side::Buy {
                (exit_px - entry_px) * size
            } else {
                (entry_px - exit_px) * size
            };

            self.realized_pnl += pnl;
            self.total_fees += fee;
            self.balance += pnl - fee;

            self.total_trades_count += 1;
            if pnl > 0.0 {
                self.winning_trades_count += 1;
            }

            self.position.side = None;
            self.position.size = 0.0;
            self.position.entry_price = 0.0;
            self.position_entry_ts = 0;
            self.target_price = 0.0;

            info!(
                "🚀 [POSITION EXITED] #{} | Closed {:?} {:.4} BTC @ ${:.2} (Hold: {}ms) | Net PnL: ${:+.2} | Reason: {}",
                exit_order_id, pos_side, size, exit_px, hold_time, pnl - fee, exit_reason
            );

            Some(OrderFill {
                timestamp: now_ms,
                order_id: exit_order_id,
                symbol: "BTC".to_string(),
                side: exit_side,
                price: exit_px,
                qty: size,
                fee,
                realized_pnl: pnl,
                is_maker: true,
            })
        } else {
            None
        }
    }

    /// Check and trigger 500ms Fast Cancel for unfilled orders
    pub fn check_timeouts(&mut self, now_ms: i64) -> Vec<(u64, String)> {
        let mut canceled = Vec::new();
        let mut expired_ids = Vec::new();

        for (id, order) in self.active_orders.iter() {
            if now_ms - order.placed_ts >= order.timeout_ms {
                expired_ids.push(*id);
                canceled.push((*id, format!("500ms Timeout Expired (Price did not reach ${:.2})", order.price)));
            }
        }

        for (id, reason) in &canceled {
            self.active_orders.remove(id);
            warn!("⏰ [FAST CANCEL TIMEOUT] Order #{} canceled: {}", id, reason);
        }

        canceled
    }

    /// Emergency Cancel All (Deadman's Switch Triggered)
    pub fn cancel_all(&mut self, reason: &str) -> Vec<u64> {
        let canceled_ids: Vec<u64> = self.active_orders.keys().cloned().collect();
        for id in &canceled_ids {
            warn!("🚨 [EMERGENCY CANCEL-ALL] Order #{} canceled: {}", id, reason);
        }
        self.active_orders.clear();
        canceled_ids
    }

    pub fn get_account(&self, mark_price: f64) -> AccountState {
        let mut unrealized = 0.0;
        if let Some(side) = self.position.side {
            if mark_price > 0.0 && self.position.size > 0.0 {
                unrealized = match side {
                    Side::Buy => (mark_price - self.position.entry_price) * self.position.size,
                    Side::Sell => (self.position.entry_price - mark_price) * self.position.size,
                };
            }
        }

        let equity = self.balance + unrealized;
        let net_pnl = equity - self.initial_balance;
        let win_rate = if self.total_trades_count > 0 {
            (self.winning_trades_count as f64 / self.total_trades_count as f64) * 100.0
        } else {
            0.0
        };

        AccountState {
            balance: self.balance,
            equity,
            unrealized_pnl: unrealized,
            realized_pnl: self.realized_pnl,
            net_pnl,
            total_fees: self.total_fees,
            total_volume: 0.0,
            win_trades: self.winning_trades_count,
            loss_trades: self.total_trades_count.saturating_sub(self.winning_trades_count),
            win_rate,
            total_trades: self.total_trades_count,
        }
    }

    pub fn get_position(&self, mark_price: f64) -> Position {
        let mut pos = self.position.clone();
        if let Some(side) = pos.side {
            if mark_price > 0.0 && pos.size > 0.0 {
                pos.unrealized_pnl = match side {
                    Side::Buy => (mark_price - pos.entry_price) * pos.size,
                    Side::Sell => (pos.entry_price - mark_price) * pos.size,
                };
                if pos.entry_price > 0.0 {
                    pos.roe_pct = (pos.unrealized_pnl / (pos.entry_price * pos.size)) * 100.0;
                }
            }
        }
        pos
    }

    pub fn active_orders_count(&self) -> usize {
        self.active_orders.len()
    }
}
