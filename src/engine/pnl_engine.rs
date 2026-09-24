use crate::engine::types::*;
use chrono::Utc;
use tracing::info;

pub struct PnlEngine {
    pub account: Account,
    pub position: Position,
    pub maker_fee_rate: f64,
    pub taker_fee_rate: f64,
    pub maintenance_margin_rate: f64,
}

impl PnlEngine {
    pub fn new(initial_balance: f64, leverage: f64, maker_fee: f64, taker_fee: f64) -> Self {
        let mut pos = Position::default();
        pos.leverage = leverage.max(1.0);
        
        Self {
            account: Account::new(initial_balance),
            position: pos,
            maker_fee_rate: maker_fee,
            taker_fee_rate: taker_fee,
            maintenance_margin_rate: 0.005, // 0.5% standard for BTC
        }
    }

    /// Process a new execution fill and update position, fees, and realized PnL
    pub fn on_fill(&mut self, fill: &mut Fill) {
        let fill_notional = fill.price * fill.qty;
        let fee_rate = match fill.liquidity {
            Liquidity::Maker => self.maker_fee_rate,
            Liquidity::Taker => self.taker_fee_rate,
        };
        let fee = fill_notional * fee_rate;
        fill.fee = fee;

        self.account.total_fees += fee;
        self.account.balance -= fee;
        self.account.total_volume += fill_notional;

        let mut fill_realized_pnl = 0.0;

        match self.position.side {
            None => {
                // Open new position
                self.position.side = Some(fill.side);
                self.position.size = fill.qty;
                self.position.entry_price = fill.price;
                self.position.margin = fill_notional / self.position.leverage;
            }
            Some(current_side) => {
                if current_side == fill.side {
                    // Increase existing position (Weighted average entry price)
                    let current_notional = self.position.entry_price * self.position.size;
                    let new_total_qty = self.position.size + fill.qty;
                    self.position.entry_price = (current_notional + fill_notional) / new_total_qty;
                    self.position.size = new_total_qty;
                    self.position.margin = (self.position.entry_price * self.position.size) / self.position.leverage;
                } else {
                    // Opposite side trade: Reduce or Flip position
                    let close_qty = fill.qty.min(self.position.size);
                    
                    // Calculate Realized PnL for closed portion
                    let pnl_per_unit = match current_side {
                        Side::Buy => fill.price - self.position.entry_price,
                        Side::Sell => self.position.entry_price - fill.price,
                    };
                    fill_realized_pnl = pnl_per_unit * close_qty;
                    
                    self.account.realized_pnl += fill_realized_pnl;
                    self.account.balance += fill_realized_pnl;
                    self.account.total_trades += 1;
                    if fill_realized_pnl > 0.0 {
                        self.account.win_trades += 1;
                    } else if fill_realized_pnl < 0.0 {
                        self.account.loss_trades += 1;
                    }

                    if fill.qty < self.position.size {
                        // Partial close
                        self.position.size -= fill.qty;
                        self.position.margin = (self.position.entry_price * self.position.size) / self.position.leverage;
                    } else if (fill.qty - self.position.size).abs() < 1e-8 {
                        // Complete close
                        self.position.side = None;
                        self.position.size = 0.0;
                        self.position.entry_price = 0.0;
                        self.position.margin = 0.0;
                        self.position.unrealized_pnl = 0.0;
                        self.position.roe_pct = 0.0;
                        self.position.liquidation_price = 0.0;
                    } else {
                        // Flip position
                        let remaining_qty = fill.qty - self.position.size;
                        self.position.side = Some(fill.side);
                        self.position.size = remaining_qty;
                        self.position.entry_price = fill.price;
                        self.position.margin = (fill.price * remaining_qty) / self.position.leverage;
                    }
                }
            }
        }

        fill.realized_pnl = fill_realized_pnl;
        info!(
            "💰 [FILL EXEC] Side: {:?} | Px: ${:.2} | Qty: {:.4} | Fee: ${:.4} | Realized PnL: ${:.2}",
            fill.side, fill.price, fill.qty, fill.fee, fill.realized_pnl
        );
    }

    /// Update Mark Price, Unrealized PnL, ROE %, and Liquidation Price
    pub fn update_market(&mut self, mark_price: f64) {
        if let Some(side) = self.position.side {
            if self.position.size > 0.0 && self.position.entry_price > 0.0 {
                let pnl = match side {
                    Side::Buy => (mark_price - self.position.entry_price) * self.position.size,
                    Side::Sell => (self.position.entry_price - mark_price) * self.position.size,
                };
                self.position.unrealized_pnl = pnl;
                
                if self.position.margin > 0.0 {
                    self.position.roe_pct = (pnl / self.position.margin) * 100.0;
                }

                // Liquidation price calculation:
                // For Long: Entry * (1 - 1/Lev + MMR)
                // For Short: Entry * (1 + 1/Lev - MMR)
                let lev_inv = 1.0 / self.position.leverage;
                self.position.liquidation_price = match side {
                    Side::Buy => (self.position.entry_price * (1.0 - lev_inv + self.maintenance_margin_rate)).max(0.0),
                    Side::Sell => self.position.entry_price * (1.0 + lev_inv - self.maintenance_margin_rate),
                };
            }
        } else {
            self.position.unrealized_pnl = 0.0;
            self.position.roe_pct = 0.0;
            self.position.liquidation_price = 0.0;
        }

        self.account.update_equity(self.position.unrealized_pnl);
    }
}
