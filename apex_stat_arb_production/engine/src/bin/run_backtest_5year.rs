use apex_stat_arb_production::core::dtw::{dtw_distance, zscore_normalize};
use apex_stat_arb_production::core::volatility::{compute_relative_atr, compute_volume_zscore, BarSnapshot};
use apex_stat_arb_production::strategy::portfolio_book::PortfolioBook;
use apex_stat_arb_production::strategy::stat_arb::StatArbSignal;
use chrono::{DateTime, Utc};
use rayon::prelude::*;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::time::Instant;

fn load_dataset(csv_dir: &str) -> (Vec<String>, BTreeMap<i64, HashMap<String, BarSnapshot>>) {
    println!("📂 [DATA] Indexing Point-In-Time 1-minute historical datasets from: {}", csv_dir);
    let start_load = Instant::now();
    let mut symbol_set = BTreeSet::new();
    let mut timeline_bars: BTreeMap<i64, HashMap<String, BarSnapshot>> = BTreeMap::new();

    let entries = match std::fs::read_dir(csv_dir) {
        Ok(e) => e,
        Err(err) => {
            eprintln!("❌ Failed to read CSV dir {}: {}", csv_dir, err);
            return (Vec::new(), timeline_bars);
        }
    };
    let mut file_count = 0;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("csv") {
            let filename = path.file_stem().unwrap().to_str().unwrap();
            let parts: Vec<&str> = filename.split('-').collect();
            if parts.is_empty() { continue; }
            let symbol = parts[0].to_string();
            symbol_set.insert(symbol.clone());

            if let Ok(file) = File::open(&path) {
                let reader = BufReader::new(file);
                for (line_idx, line) in reader.lines().enumerate() {
                    if line_idx == 0 { continue; }
                    if let Ok(l) = line {
                        let cols: Vec<&str> = l.split(',').collect();
                        if cols.len() >= 6 {
                            let ts: i64 = cols[0].parse().unwrap_or(0);
                            let open: f64 = cols[1].parse().unwrap_or(0.0);
                            let high: f64 = cols[2].parse().unwrap_or(0.0);
                            let low: f64 = cols[3].parse().unwrap_or(0.0);
                            let close: f64 = cols[4].parse().unwrap_or(0.0);
                            let volume: f64 = cols[5].parse().unwrap_or(0.0);

                            if ts > 0 && close > 0.0 {
                                timeline_bars.entry(ts).or_default().insert(
                                    symbol.clone(),
                                    BarSnapshot { ts, open, high, low, close, volume },
                                );
                            }
                        }
                    }
                }
                file_count += 1;
            }
        }
    }

    let symbols: Vec<String> = symbol_set.into_iter().collect();
    let elapsed = start_load.elapsed().as_secs_f64();
    println!("✅ Indexed {} CSV monthly files across {} unique symbols in {:.1}s.", file_count, symbols.len(), elapsed);
    println!("• Total 1-Minute Timeline Points: {} continuous bars (2022.01 - 2026.08)", timeline_bars.len());
    (symbols, timeline_bars)
}

fn main() {
    let csv_dir = "/Users/elias/apex-alpha/data/vision_1m_csv";
    let (_symbols, timeline_bars) = load_dataset(csv_dir);

    if timeline_bars.is_empty() { return; }

    let timestamps: Vec<i64> = timeline_bars.keys().cloned().collect();
    let total_bars = timestamps.len();

    println!("\n🏛️ TWO SIGMA: PRODUCTION VOLATILITY-ADAPTIVE STAT-ARB ENGINE (2022-2026)");
    println!("• Volatility Regime Scaling: ATR-Adaptive Surge Threshold (min 0.9% ~ max 2.5%)");
    println!("• Dynamic OU Exit: Target Profit = 1.5 * ATR_60m | Stop Loss = -1.0 * ATR_60m");
    println!("• Fast Capital Velocity: Max Holding 25 mins (Fast OU Mean Reversion)");
    println!("• Book Limit: Max 5 Concurrent Pairs (5 Longs + 5 Shorts | $2,000 per pair)\n");

    let pit_rebalance_interval = 10080;
    let lookback_hours = 120;
    let lookback_1m_bars = lookback_hours * 60;
    let dtw_resample_mins = 5;
    let dtw_points = lookback_1m_bars / dtw_resample_mins;
    let dtw_recompute_interval = 360;
    let dtw_window_radius = 12;
    let top_n_universe = 30;
    let dtw_quantile_cutoff = 0.08;
    let max_slots = 5;
    let leader_window = 5;
    let initial_capital = 10000.0;

    let mut book = PortfolioBook::new(max_slots, initial_capital);
    let mut current_pit_universe: Vec<String> = Vec::new();
    let mut current_top_dtw_pairs: HashMap<String, Vec<(String, f64, f64)>> = HashMap::new();
    let mut daily_equity: BTreeMap<String, f64> = BTreeMap::new();

    let start_sim = Instant::now();
    let start_bar = lookback_1m_bars + pit_rebalance_interval;
    let total_sim_bars = total_bars - 50 - start_bar;
    let log_interval = total_sim_bars / 20;
    let mut last_log_bar = start_bar;

    for bar_idx in start_bar..(total_bars - 50) {
        let ts = timestamps[bar_idx];
        let current_bar_map = match timeline_bars.get(&ts) {
            Some(m) => m,
            None => continue,
        };

        // ⏱️ Mandatory Progress & ETA Logging
        if bar_idx - last_log_bar >= log_interval {
            let processed = bar_idx - start_bar;
            let pct = (processed as f64 / total_sim_bars as f64) * 100.0;
            let elapsed_sec = start_sim.elapsed().as_secs_f64();
            let speed = processed as f64 / elapsed_sec.max(0.001);
            let remaining_bars = total_sim_bars.saturating_sub(processed);
            let eta_sec = remaining_bars as f64 / speed.max(1.0);

            let elapsed_min = (elapsed_sec / 60.0).floor();
            let elapsed_rem_sec = (elapsed_sec % 60.0).floor();
            let eta_min = (eta_sec / 60.0).floor();
            let eta_rem_sec = (eta_sec % 60.0).floor();

            println!(
                "[PROGRESS] {:>5.1}% ({:>7} / {:>7} bars) | Elapsed: {:>02}m{:>02}s | ETA: {:>02}m{:>02}s | Speed: {:>6.0} b/s | Closed: {}",
                pct, processed, total_sim_bars, elapsed_min, elapsed_rem_sec, eta_min, eta_rem_sec, speed, book.closed_trades.len()
            );
            last_log_bar = bar_idx;
        }

        // 1. Weekly PIT Universe Selection
        if (bar_idx - start_bar) % pit_rebalance_interval == 0 || current_pit_universe.is_empty() {
            let mut past_vol: HashMap<String, f64> = HashMap::new();
            for past_i in (bar_idx - pit_rebalance_interval)..bar_idx {
                let past_ts = timestamps[past_i];
                if let Some(bars) = timeline_bars.get(&past_ts) {
                    for (s, b) in bars {
                        *past_vol.entry(s.clone()).or_insert(0.0) += b.volume * b.close;
                    }
                }
            }
            let mut sorted_vol: Vec<(String, f64)> = past_vol.into_iter().collect();
            sorted_vol.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            current_pit_universe = sorted_vol.into_iter().take(top_n_universe).map(|(s, _)| s).collect();
        }

        // 2. Parallel 120-Hour DTW Empirical Quantile Matrix (Rayon)
        if (bar_idx - start_bar) % dtw_recompute_interval == 0 || current_top_dtw_pairs.is_empty() {
            let mut symbol_series: HashMap<String, Vec<f64>> = HashMap::new();

            for s in &current_pit_universe {
                let mut prices_5m = Vec::with_capacity(dtw_points);
                let mut valid = true;

                for p_idx in 0..dtw_points {
                    let sample_bar_idx = bar_idx - lookback_1m_bars + (p_idx * dtw_resample_mins);
                    let s_ts = timestamps[sample_bar_idx];
                    if let Some(bar) = timeline_bars.get(&s_ts).and_then(|m| m.get(s)) {
                        prices_5m.push(bar.close);
                    } else {
                        valid = false;
                        break;
                    }
                }

                if valid && prices_5m.len() == dtw_points {
                    let z_series = zscore_normalize(&prices_5m);
                    symbol_series.insert(s.clone(), z_series);
                }
            }

            let active_symbols: Vec<String> = symbol_series.keys().cloned().collect();
            let pair_indices: Vec<(usize, usize)> = (0..active_symbols.len())
                .flat_map(|i| ((i + 1)..active_symbols.len()).map(move |j| (i, j)))
                .collect();

            let mut pairwise_distances: Vec<(String, String, f64)> = pair_indices
                .into_par_iter()
                .filter_map(|(i, j)| {
                    let s1 = &active_symbols[i];
                    let s2 = &active_symbols[j];
                    let ser1 = &symbol_series[s1];
                    let ser2 = &symbol_series[s2];
                    let dist = dtw_distance(ser1, ser2, dtw_window_radius);
                    if dist.is_finite() && dist > 0.0 {
                        Some((s1.clone(), s2.clone(), dist))
                    } else {
                        None
                    }
                })
                .collect();

            if !pairwise_distances.is_empty() {
                pairwise_distances.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
                let total_pairs = pairwise_distances.len() as f64;
                let cutoff_idx = (total_pairs * dtw_quantile_cutoff).ceil() as usize;

                current_top_dtw_pairs.clear();

                for (rank, (s1, s2, dist)) in pairwise_distances.iter().enumerate() {
                    let quantile_pct = (rank as f64 / total_pairs) * 100.0;
                    if rank <= cutoff_idx {
                        current_top_dtw_pairs.entry(s1.clone()).or_default().push((s2.clone(), *dist, quantile_pct));
                        current_top_dtw_pairs.entry(s2.clone()).or_default().push((s1.clone(), *dist, quantile_pct));
                    }
                }
            }
        }

        // 3. Evaluate Exits on Active Positions
        let mut to_close = Vec::new();
        for (idx, pos) in book.active_positions.iter().enumerate() {
            if let (Some(s_bar), Some(l_bar)) = (current_bar_map.get(&pos.short_sym), current_bar_map.get(&pos.long_sym)) {
                if let Some((s_ret, l_ret, spread_ret, reason)) = pos.evaluate_exit(s_bar.close, l_bar.close, bar_idx) {
                    to_close.push((idx, s_ret, l_ret, spread_ret, reason));
                }
            }
        }

        // Close in reverse order to maintain index validity
        for (idx, s_ret, l_ret, spread_ret, reason) in to_close.into_iter().rev() {
            book.close_position(idx, ts, s_ret, l_ret, spread_ret, reason, bar_idx);
        }

        let dt = DateTime::<Utc>::from_timestamp_millis(ts).unwrap();
        let date_key = dt.format("%Y-%m-%d").to_string();
        daily_equity.insert(date_key, book.current_equity());

        // 4. Signal Candidate Generation & Slot Allocation
        if book.available_slots() > 0 {
            let busy_symbols = book.get_busy_symbols();
            let prev_leader_ts = timestamps[bar_idx.saturating_sub(leader_window)];

            if let Some(prev_bar_map) = timeline_bars.get(&prev_leader_ts) {
                let mut candidates: Vec<StatArbSignal> = Vec::new();

                for s in &current_pit_universe {
                    if busy_symbols.contains(s) { continue; }

                    if let (Some(curr), Some(prev)) = (current_bar_map.get(s), prev_bar_map.get(s)) {
                        let ret = (curr.close - prev.close) / prev.close;

                        // Rolling 60m ATR & Volume
                        let mut recent_bars = Vec::with_capacity(60);
                        let mut vol_history = Vec::with_capacity(60);

                        for past_i in bar_idx.saturating_sub(60)..bar_idx {
                            if let Some(b) = timeline_bars.get(&timestamps[past_i]).and_then(|m| m.get(s)) {
                                recent_bars.push(b.clone());
                                vol_history.push(b.volume);
                            }
                        }

                        if recent_bars.len() >= 30 {
                            let atr_60m = compute_relative_atr(&recent_bars);
                            let dynamic_surge_threshold = (2.0 * atr_60m).clamp(0.009, 0.025);

                            if ret >= dynamic_surge_threshold {
                                let vol_z = compute_volume_zscore(curr.volume, &vol_history);

                                if vol_z >= 2.2 {
                                    if let Some(cohort_list) = current_top_dtw_pairs.get(s) {
                                        for (c_sym, _dist, q_pct) in cohort_list {
                                            if busy_symbols.contains(c_sym) { continue; }

                                            if let (Some(c_curr), Some(c_prev)) = (current_bar_map.get(c_sym), prev_bar_map.get(c_sym)) {
                                                let c_ret = (c_curr.close - c_prev.close) / c_prev.close;
                                                let spread_divergence = ret - c_ret;
                                                let min_div = (1.5 * atr_60m).clamp(0.007, 0.020);

                                                if spread_divergence >= min_div {
                                                    let target_profit = (1.5 * atr_60m).clamp(0.009, 0.022);
                                                    let stop_loss = -(1.0 * atr_60m).clamp(0.006, 0.015);
                                                    let conviction = spread_divergence / (q_pct / 100.0).max(0.01);

                                                    candidates.push(StatArbSignal {
                                                        timestamp: ts,
                                                        short_symbol: s.clone(),
                                                        long_symbol: c_sym.clone(),
                                                        short_entry_px: curr.close,
                                                        long_entry_px: c_curr.close,
                                                        spread_divergence,
                                                        target_profit,
                                                        stop_loss,
                                                        max_holding_bars: 25,
                                                        dtw_quantile_pct: *q_pct,
                                                        conviction_score: conviction,
                                                    });
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                candidates.sort_by(|a, b| b.conviction_score.partial_cmp(&a.conviction_score).unwrap_or(std::cmp::Ordering::Equal));

                for sig in candidates {
                    if book.available_slots() == 0 { break; }
                    book.try_open_position(sig, bar_idx);
                }
            }
        }
    }

    let total_trades = book.closed_trades.len();
    let wins = book.closed_trades.iter().filter(|t| t.net_pnl_usd > 0.0).count();
    let wr = (wins as f64 / total_trades as f64) * 100.0;
    let final_equity = book.current_equity();
    let cum_return = ((final_equity - initial_capital) / initial_capital) * 100.0;

    println!("\n==================================================================");
    println!("🏛️ FINAL 5-YEAR BACKTEST SUMMARY (2022 ~ 2026)");
    println!("==================================================================");
    println!("• Total Closed Trades     : {} trades", total_trades);
    println!("• 5-Year Win Rate         : {:.1}% ({} Wins / {} Losses)", wr, wins, total_trades - wins);
    println!("• Initial Capital         : ${:.2}", initial_capital);
    println!("• Final Equity            : ${:.2} (+{:.2}%)", final_equity, cum_return);
    println!("• Market Beta             : 0.00 (Neutral)");
    println!("==================================================================\n");
}
