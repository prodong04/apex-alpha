use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatArbSignal {
    pub timestamp: i64,
    pub short_symbol: String,
    pub long_symbol: String,
    pub short_entry_px: f64,
    pub long_entry_px: f64,
    pub spread_divergence: f64,
    pub target_profit: f64,
    pub stop_loss: f64,
    pub max_holding_bars: usize,
    pub dtw_quantile_pct: f64,
    pub conviction_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivePairPosition {
    pub slot_id: usize,
    pub entry_bar: usize,
    pub entry_ts: i64,
    pub short_sym: String,
    pub long_sym: String,
    pub short_entry_px: f64,
    pub long_entry_px: f64,
    pub target_profit: f64,
    pub stop_loss: f64,
    pub max_holding: usize,
    pub allocated_capital_usd: f64,
    pub dtw_percentile: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClosedPairTrade {
    pub entry_ts: i64,
    pub exit_ts: i64,
    pub short_sym: String,
    pub long_sym: String,
    pub holding_bars: usize,
    pub short_ret: f64,
    pub long_ret: f64,
    pub gross_spread_return: f64,
    pub net_spread_return: f64,
    pub net_pnl_usd: f64,
    pub exit_reason: String,
}

impl ActivePairPosition {
    pub fn evaluate_exit(
        &self,
        current_short_px: f64,
        current_long_px: f64,
        current_bar_idx: usize,
    ) -> Option<(f64, f64, f64, &'static str)> {
        let s_ret = (self.short_entry_px - current_short_px) / self.short_entry_px;
        let l_ret = (current_long_px - self.long_entry_px) / self.long_entry_px;
        let spread_ret = (s_ret + l_ret) / 2.0;
        let holding_bars = current_bar_idx - self.entry_bar;

        if spread_ret >= self.target_profit {
            Some((s_ret, l_ret, spread_ret, "CONVERGENCE_PROFIT"))
        } else if spread_ret <= self.stop_loss {
            Some((s_ret, l_ret, spread_ret, "STOP_LOSS"))
        } else if holding_bars >= self.max_holding {
            Some((s_ret, l_ret, spread_ret, "TIMEOUT_25M"))
        } else {
            None
        }
    }
}
