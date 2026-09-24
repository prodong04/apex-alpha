/// Volatility and Surge Measurement Engines

#[derive(Debug, Clone)]
pub struct BarSnapshot {
    pub ts: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

/// Computes rolling Average True Range (ATR) normalized by price
pub fn compute_relative_atr(bars: &[BarSnapshot]) -> f64 {
    if bars.is_empty() {
        return 0.005; // 50 bps fallback default
    }
    let mut sum_range = 0.0;
    let mut valid_count = 0;

    for b in bars {
        if b.close > 0.0 {
            sum_range += (b.high - b.low) / b.close;
            valid_count += 1;
        }
    }

    if valid_count > 0 {
        sum_range / valid_count as f64
    } else {
        0.005
    }
}

/// Computes volume Z-score
pub fn compute_volume_zscore(current_vol: f64, history_vols: &[f64]) -> f64 {
    let n = history_vols.len();
    if n < 10 {
        return 0.0;
    }
    let mean = history_vols.iter().sum::<f64>() / n as f64;
    let var = history_vols.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1) as f64;
    let std = var.sqrt();
    if std > 1e-6 {
        (current_vol - mean) / std
    } else {
        0.0
    }
}
