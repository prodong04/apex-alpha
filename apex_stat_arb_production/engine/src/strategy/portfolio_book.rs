use super::stat_arb::{ActivePairPosition, ClosedPairTrade, StatArbSignal};
use std::collections::HashSet;

pub struct PortfolioBook {
    pub max_slots: usize,
    pub total_capital_usd: f64,
    pub slot_capital_usd: f64,
    pub active_positions: Vec<ActivePairPosition>,
    pub closed_trades: Vec<ClosedPairTrade>,
    pub running_pnl_usd: f64,
}

impl PortfolioBook {
    pub fn new(max_slots: usize, total_capital_usd: f64) -> Self {
        let slot_capital_usd = total_capital_usd / max_slots as f64;
        Self {
            max_slots,
            total_capital_usd,
            slot_capital_usd,
            active_positions: Vec::with_capacity(max_slots),
            closed_trades: Vec::new(),
            running_pnl_usd: 0.0,
        }
    }

    pub fn available_slots(&self) -> usize {
        self.max_slots.saturating_sub(self.active_positions.len())
    }

    pub fn get_busy_symbols(&self) -> HashSet<String> {
        let mut busy = HashSet::new();
        for pos in &self.active_positions {
            busy.insert(pos.short_sym.clone());
            busy.insert(pos.long_sym.clone());
        }
        busy
    }

    pub fn try_open_position(&mut self, sig: StatArbSignal, bar_idx: usize) -> bool {
        if self.active_positions.len() >= self.max_slots {
            return false;
        }

        let busy = self.get_busy_symbols();
        if busy.contains(&sig.short_symbol) || busy.contains(&sig.long_symbol) {
            return false;
        }

        self.active_positions.push(ActivePairPosition {
            slot_id: self.active_positions.len(),
            entry_bar: bar_idx,
            entry_ts: sig.timestamp,
            short_sym: sig.short_symbol,
            long_sym: sig.long_symbol,
            short_entry_px: sig.short_entry_px,
            long_entry_px: sig.long_entry_px,
            target_profit: sig.target_profit,
            stop_loss: sig.stop_loss,
            max_holding: sig.max_holding_bars,
            allocated_capital_usd: self.slot_capital_usd,
            dtw_percentile: sig.dtw_quantile_pct,
        });

        true
    }

    pub fn close_position(
        &mut self,
        pos_idx: usize,
        exit_ts: i64,
        s_ret: f64,
        l_ret: f64,
        spread_ret: f64,
        reason: &str,
        current_bar_idx: usize,
    ) {
        let pos = self.active_positions.remove(pos_idx);
        let net_return = spread_ret; // Maker Post-Only (0 bps friction)
        let pnl = pos.allocated_capital_usd * net_return;
        self.running_pnl_usd += pnl;

        self.closed_trades.push(ClosedPairTrade {
            entry_ts: pos.entry_ts,
            exit_ts,
            short_sym: pos.short_sym,
            long_sym: pos.long_sym,
            holding_bars: current_bar_idx - pos.entry_bar,
            short_ret: s_ret,
            long_ret: l_ret,
            gross_spread_return: spread_ret,
            net_spread_return: net_return,
            net_pnl_usd: pnl,
            exit_reason: reason.to_string(),
        });
    }

    pub fn current_equity(&self) -> f64 {
        self.total_capital_usd + self.running_pnl_usd
    }
}
