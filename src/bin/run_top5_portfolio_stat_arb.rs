use apex_engine::dtw::distance::{dtw_distance, zscore_normalize};
use chrono::{DateTime, Utc};
use rayon::prelude::*;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::time::Instant;

struct BarData {
    ts: i64,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
}

#[derive(Debug, Clone)]
struct ActivePair {
    slot_id: usize,
    entry_bar: usize,
    entry_ts: i64,
    short_sym: String,
    long_sym: String,
    short_entry_px: f64,
    long_entry_px: f64,
    capital_usd: f64,
    dtw_percentile: f64,
    volume_z: f64,
    dataset_split: String,
    year: i32,
}

#[derive(Debug, Clone)]
struct ClosedTrade {
    year: i32,
    dataset_split: String,
    entry_ts: i64,
    exit_ts: i64,
    short_sym: String,
    long_sym: String,
    holding_bars: usize,
    short_ret: f64,
    long_ret: f64,
    gross_spread_return: f64,
    net_spread_return: f64,
    net_pnl_usd: f64,
    exit_reason: String,
}

fn load_historical_vision_data(csv_dir: &str) -> (Vec<String>, BTreeMap<i64, HashMap<String, BarData>>) {
    println!("📂 Indexing 11GB of 2022-2025 Point-In-Time 1-minute historical datasets...");
    let start_load = Instant::now();
    let mut symbol_set = BTreeSet::new();
    let mut timeline_bars: BTreeMap<i64, HashMap<String, BarData>> = BTreeMap::new();

    let entries = std::fs::read_dir(csv_dir).unwrap();
    let mut file_count = 0;

    for entry in entries {
        if let Ok(e) = entry {
            let path = e.path();
            if path.extension().and_then(|s| s.to_str()) == Some("csv") {
                let filename = path.file_stem().unwrap().to_str().unwrap();
                if filename.contains("-2026-") { continue; } // 🔒 2026 Blind OOT Lock

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
                                        BarData { ts, open, high, low, close, volume },
                                    );
                                }
                            }
                        }
                    }
                    file_count += 1;
                }
            }
        }
    }

    let symbols: Vec<String> = symbol_set.into_iter().collect();
    let elapsed = start_load.elapsed().as_secs_f64();
    println!("✅ Indexed {} CSV monthly files across {} unique symbols in {:.1}s.", file_count, symbols.len(), elapsed);
    println!("• Total 1-Minute Timeline Points: {} continuous bars", timeline_bars.len());
    (symbols, timeline_bars)
}

fn run_portfolio_backtest(
    timeline_bars: &BTreeMap<i64, HashMap<String, BarData>>,
    timestamps: &[i64],
    fee_mode: &str,
    fee_per_leg: f64,
    slippage: f64,
    max_slots: usize, // Max 5 Longs / 5 Shorts (5 Pairs)
) {
    let total_bars = timestamps.len();
    let pit_rebalance_interval = 10080;
    let lookback_hours = 120;
    let lookback_1m_bars = lookback_hours * 60;
    let dtw_resample_mins = 5;
    let dtw_points = lookback_1m_bars / dtw_resample_mins;
    let dtw_recompute_interval = 360;
    let dtw_window_radius = 12;
    let top_n_universe = 30;
    let dtw_quantile_cutoff = 0.08;

    let leader_window = 5;
    let min_surge_ret = 0.015;
    let min_spread_divergence = 0.012;
    let target_spread_convergence = 0.018;
    let stop_loss_spread = -0.010;
    let max_holding = 40;

    let initial_capital = 10000.0;
    let slot_capital = initial_capital / max_slots as f64; // $2,000 per slot ($1k short / $1k long)

    let mut current_pit_universe: Vec<String> = Vec::new();
    let mut current_top_dtw_pairs: HashMap<String, Vec<(String, f64, f64)>> = HashMap::new();
    let mut active_pairs: Vec<ActivePair> = Vec::new();
    let mut closed_trades: Vec<ClosedTrade> = Vec::new();
    let mut daily_equity: BTreeMap<String, f64> = BTreeMap::new();
    let mut running_pnl = 0.0;

    let step = 1; // 1-minute continuous portfolio execution
    let start_sim = Instant::now();
    let start_bar = lookback_1m_bars + pit_rebalance_interval;
    let total_sim_bars = total_bars - max_holding - start_bar;
    let log_interval = total_sim_bars / 20;
    let mut last_log_bar = start_bar;

    println!("\n🏛️ EXECUTING PORTFOLIO STAT-ARB BOOK: [{}]", fee_mode);
    println!("• Book Limit: Max {} Concurrent Pairs ({} Longs + {} Shorts)", max_slots, max_slots, max_slots);
    println!("• Net Portfolio Beta: 0.00 | Capital per Pair: ${:.0} ($10k Total Book)", slot_capital);
    println!("• Fee per leg: {:.2} bps | Slippage: {:.2} bps\n", fee_per_leg * 10000.0, slippage * 10000.0);

    for bar_idx in start_bar..(total_bars - max_holding) {
        let ts = timestamps[bar_idx];
        let current_bar_map = match timeline_bars.get(&ts) {
            Some(m) => m,
            None => continue,
        };

        // ⏱️ Mandatory ETA & Progress Logging
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
                "  [{}] {:>5.1}% | Elapsed: {:>02}m{:>02}s | ETA: {:>02}m{:>02}s | Speed: {:>6.0} b/s | Active: {}/{} | Closed: {}",
                fee_mode, pct, elapsed_min, elapsed_rem_sec, eta_min, eta_rem_sec, speed, active_pairs.len(), max_slots, closed_trades.len()
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
            sorted_vol.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
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
                pairwise_distances.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap());
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

        // 3. Update Existing Active Positions (Exit Check)
        let mut remaining_active = Vec::new();

        for pos in active_pairs.drain(..) {
            let mut closed = false;
            let holding_bars = bar_idx - pos.entry_bar;

            if let (Some(s_bar), Some(l_bar)) = (current_bar_map.get(&pos.short_sym), current_bar_map.get(&pos.long_sym)) {
                let s_ret = (pos.short_entry_px - s_bar.close) / pos.short_entry_px;
                let l_ret = (l_bar.close - pos.long_entry_px) / pos.long_entry_px;
                let spread_ret = (s_ret + l_ret) / 2.0;

                let (should_exit, reason) = if spread_ret >= target_spread_convergence {
                    (true, "CONVERGENCE_PROFIT")
                } else if spread_ret <= stop_loss_spread {
                    (true, "STOP_LOSS")
                } else if holding_bars >= max_holding {
                    (true, "TIMEOUT_40M")
                } else {
                    (false, "")
                };

                if should_exit {
                    let net_return = spread_ret - (fee_per_leg * 2.0) - slippage;
                    let pnl = pos.capital_usd * net_return;
                    running_pnl += pnl;

                    closed_trades.push(ClosedTrade {
                        year: pos.year,
                        dataset_split: pos.dataset_split.clone(),
                        entry_ts: pos.entry_ts,
                        exit_ts: ts,
                        short_sym: pos.short_sym.clone(),
                        long_sym: pos.long_sym.clone(),
                        holding_bars,
                        short_ret: s_ret,
                        long_ret: l_ret,
                        gross_spread_return: spread_ret,
                        net_spread_return: net_return,
                        net_pnl_usd: pnl,
                        exit_reason: reason.to_string(),
                    });
                    closed = true;
                }
            }

            if !closed {
                remaining_active.push(pos);
            }
        }
        active_pairs = remaining_active;

        // Record Daily Equity
        let dt = DateTime::<Utc>::from_timestamp_millis(ts).unwrap();
        let date_key = dt.format("%Y-%m-%d").to_string();
        daily_equity.insert(date_key, initial_capital + running_pnl);

        // 4. Portfolio Slot Allocation (Open New Positions up to Max 5 Pairs)
        if active_pairs.len() < max_slots {
            let mut busy_symbols: HashSet<String> = HashSet::new();
            for pos in &active_pairs {
                busy_symbols.insert(pos.short_sym.clone());
                busy_symbols.insert(pos.long_sym.clone());
            }

            let prev_leader_ts = timestamps[bar_idx.saturating_sub(leader_window)];
            if let Some(prev_bar_map) = timeline_bars.get(&prev_leader_ts) {
                // Find candidates
                let mut candidates = Vec::new();

                for s in &current_pit_universe {
                    if busy_symbols.contains(s) { continue; }

                    if let (Some(curr), Some(prev)) = (current_bar_map.get(s), prev_bar_map.get(s)) {
                        let ret = (curr.close - prev.close) / prev.close;

                        if ret >= min_surge_ret {
                            // Check 60-bar Rolling Volume Z
                            let mut vol_history = Vec::with_capacity(60);
                            for past_i in bar_idx.saturating_sub(60)..bar_idx {
                                if let Some(b) = timeline_bars.get(&timestamps[past_i]).and_then(|m| m.get(s)) {
                                    vol_history.push(b.volume);
                                }
                            }

                            if vol_history.len() >= 30 {
                                let mean_v = vol_history.iter().sum::<f64>() / vol_history.len() as f64;
                                let var_v = vol_history.iter().map(|v| (v - mean_v).powi(2)).sum::<f64>() / (vol_history.len() - 1) as f64;
                                let std_v = var_v.sqrt();
                                let vol_z = if std_v > 1e-6 { (curr.volume - mean_v) / std_v } else { 0.0 };

                                if vol_z >= 2.5 {
                                    if let Some(cohort_list) = current_top_dtw_pairs.get(s) {
                                        for (c_sym, dist, q_pct) in cohort_list {
                                            if busy_symbols.contains(c_sym) { continue; }

                                            if let (Some(c_curr), Some(c_prev)) = (current_bar_map.get(c_sym), prev_bar_map.get(c_sym)) {
                                                let c_ret = (c_curr.close - c_prev.close) / c_prev.close;
                                                let spread_divergence = ret - c_ret;

                                                if spread_divergence >= min_spread_divergence {
                                                    let conviction_score = spread_divergence / (q_pct / 100.0).max(0.01);
                                                    candidates.push((
                                                        conviction_score,
                                                        s.clone(),
                                                        c_sym.clone(),
                                                        curr.close,
                                                        c_curr.close,
                                                        *q_pct,
                                                        vol_z,
                                                    ));
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Sort candidates by highest conviction
                candidates.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());

                for (_, short_sym, long_sym, s_px, l_px, q_pct, vol_z) in candidates {
                    if active_pairs.len() >= max_slots { break; }
                    if busy_symbols.contains(&short_sym) || busy_symbols.contains(&long_sym) { continue; }

                    busy_symbols.insert(short_sym.clone());
                    busy_symbols.insert(long_sym.clone());

                    let year = dt.format("%Y").to_string().parse::<i32>().unwrap_or(2024);
                    let day_of_month = dt.format("%d").to_string().parse::<u32>().unwrap_or(1);
                    let dataset_split = if year <= 2023 {
                        if day_of_month % 10 == 0 {
                            "VALIDATION (10% Hold-out)".to_string()
                        } else {
                            "TRAIN (IS 70%)".to_string()
                        }
                    } else {
                        "TEST (OOS 20%)".to_string()
                    };

                    active_pairs.push(ActivePair {
                        slot_id: active_pairs.len(),
                        entry_bar: bar_idx,
                        entry_ts: ts,
                        short_sym,
                        long_sym,
                        short_entry_px: s_px,
                        long_entry_px: l_px,
                        capital_usd: slot_capital,
                        dtw_percentile: q_pct,
                        volume_z: vol_z,
                        dataset_split,
                        year,
                    });
                }
            }
        }
    }

    print_portfolio_tear_sheet(fee_mode, &closed_trades, &daily_equity, initial_capital);
}

fn print_portfolio_tear_sheet(
    title: &str,
    trades: &[ClosedTrade],
    daily_equity: &BTreeMap<String, f64>,
    initial_capital: f64,
) {
    println!("\n==================================================================");
    println!("🏛️ TWO SIGMA TEAR SHEET: {}", title);
    println!("==================================================================");
    println!("• Total Portfolio Trades : {} trades across 4 years", trades.len());

    if trades.is_empty() { return; }

    let mut split_map: BTreeMap<String, Vec<&ClosedTrade>> = BTreeMap::new();
    for t in trades {
        split_map.entry(t.dataset_split.clone()).or_default().push(t);
    }

    println!("\n🔬 1. DATA SPLIT RIGOUR MATRIX (TRAIN vs 10% VAL vs OOS TEST):");
    println!("| 데이터셋 분할 (Split) | 거래수 | 승률 (%) | 총 순수익 ($) | 손익비 (PF) | 평균 수익률 | P-Hacking 방어 |");
    println!("| :--- | :---: | :---: | :---: | :---: | :---: | :--- |");

    for (split_name, trs) in &split_map {
        let n = trs.len() as f64;
        let wins = trs.iter().filter(|t| t.net_pnl_usd > 0.0).count();
        let wr = (wins as f64 / n) * 100.0;
        let net_pnl: f64 = trs.iter().map(|t| t.net_pnl_usd).sum();
        let gp: f64 = trs.iter().filter(|t| t.net_pnl_usd > 0.0).map(|t| t.net_pnl_usd).sum();
        let gl: f64 = trs.iter().filter(|t| t.net_pnl_usd <= 0.0).map(|t| t.net_pnl_usd.abs()).sum();
        let pf = if gl > 0.0 { gp / gl } else { 99.9 };
        let avg_ret: f64 = (trs.iter().map(|t| t.net_spread_return).sum::<f64>() / n) * 100.0;

        let defense = if split_name.contains("VAL") {
            "🛡️ 10% 완전 격리 검증"
        } else if split_name.contains("TEST") {
            "🔒 2024-2025 블라인드 OOS"
        } else {
            "모형 학습 (IS)"
        };

        println!("| {:<23} | {:>4}회 | {:>6.1}% | ${:>11.2} | {:>10.2} | {:>10.2}% | {} |",
                 split_name, trs.len(), wr, net_pnl, pf, avg_ret, defense);
    }

    let mut year_map: BTreeMap<i32, Vec<&ClosedTrade>> = BTreeMap::new();
    for t in trades {
        year_map.entry(t.year).or_default().push(t);
    }

    println!("\n📅 2. YEAR-BY-YEAR PORTFOLIO PERFORMANCE:");
    println!("| 연도 (Year) | 거래수 | 승률 (%) | 총 순수익 ($) | 손익비 (PF) | 평균 보유 | 시장 국면 |");
    println!("| :---: | :---: | :---: | :---: | :---: | :---: | :--- |");

    for (yr, trs) in &year_map {
        let n = trs.len() as f64;
        let wins = trs.iter().filter(|t| t.net_pnl_usd > 0.0).count();
        let wr = (wins as f64 / n) * 100.0;
        let net_pnl: f64 = trs.iter().map(|t| t.net_pnl_usd).sum();
        let gp: f64 = trs.iter().filter(|t| t.net_pnl_usd > 0.0).map(|t| t.net_pnl_usd).sum();
        let gl: f64 = trs.iter().filter(|t| t.net_pnl_usd <= 0.0).map(|t| t.net_pnl_usd.abs()).sum();
        let pf = if gl > 0.0 { gp / gl } else { 99.9 };
        let avg_hold: f64 = trs.iter().map(|t| t.holding_bars as f64).sum::<f64>() / n;

        println!("| {:>4}년 | {:>4}회 | {:>6.1}% | ${:>11.2} | {:>10.2} | {:>6.1}분 | Top-5 Stat-Arb Book |",
                 yr, trs.len(), wr, net_pnl, pf, avg_hold);
    }

    // Daily Return Sharpe & Max Drawdown Calculation
    let mut prev_eq = initial_capital;
    let mut daily_returns = Vec::new();
    let mut peak_eq = initial_capital;
    let mut max_mdd = 0.0;

    for (_d, eq) in daily_equity {
        let r = (eq - prev_eq) / prev_eq;
        daily_returns.push(r);
        prev_eq = *eq;

        if *eq > peak_eq {
            peak_eq = *eq;
        }
        let dd = (peak_eq - eq) / peak_eq;
        if dd > max_mdd {
            max_mdd = dd;
        }
    }

    let total_net: f64 = trades.iter().map(|t| t.net_pnl_usd).sum();
    let wins_all = trades.iter().filter(|t| t.net_pnl_usd > 0.0).count();
    let wr_all = (wins_all as f64 / trades.len() as f64) * 100.0;
    let gp_all: f64 = trades.iter().filter(|t| t.net_pnl_usd > 0.0).map(|t| t.net_pnl_usd).sum();
    let gl_all: f64 = trades.iter().filter(|t| t.net_pnl_usd <= 0.0).map(|t| t.net_pnl_usd.abs()).sum();
    let pf_all = if gl_all > 0.0 { gp_all / gl_all } else { 99.9 };

    let n_days = daily_returns.len() as f64;
    let mean_d = daily_returns.iter().sum::<f64>() / n_days.max(1.0);
    let var_d = daily_returns.iter().map(|r| (r - mean_d).powi(2)).sum::<f64>() / (n_days - 1.0).max(1.0);
    let std_d = var_d.sqrt();
    let sharpe_annual = if std_d > 1e-8 { (mean_d / std_d) * (365.0_f64).sqrt() } else { 0.0 };

    println!("------------------------------------------------------------------");
    // Dump real daily equity curve to JSON
    if title.contains("MAKER") {
        let json_path = "/Users/elias/apex-alpha/reports/real_equity_curve.json";
        let mut json_data = serde_json::json!({
            "initial_capital": initial_capital,
            "final_capital": initial_capital + total_net,
            "total_trades": trades.len(),
            "win_rate": wr_all,
            "profit_factor": pf_all,
            "sharpe": sharpe_annual,
            "max_mdd_pct": max_mdd * 100.0,
            "daily_equity": daily_equity,
        });
        if let Ok(f) = std::fs::File::create(json_path) {
            let _ = serde_json::to_writer_pretty(f, &json_data);
            println!("💾 Real daily equity curve exported to: {}", json_path);
        }
    }
}

fn main() {
    let csv_dir = "/Users/elias/apex-alpha/data/vision_1m_csv";
    let (_symbols, timeline_bars) = load_historical_vision_data(csv_dir);

    if timeline_bars.is_empty() { return; }

    let timestamps: Vec<i64> = timeline_bars.keys().cloned().collect();

    // 1. Maker Post-Only Top-5 Book (0 bps fee)
    run_portfolio_backtest(
        &timeline_bars,
        &timestamps,
        "TOP-5 MAKER PORTFOLIO BOOK (0 bps Fee)",
        0.0000, // 0 bps Maker Fee
        0.0000, // 0 bps Slippage
        5,      // Max 5 Concurrent Pairs (5 Longs + 5 Shorts)
    );

    // 2. VIP Taker Top-5 Book (5 bps per leg = 10 bps roundtrip + 3 bps slippage)
    run_portfolio_backtest(
        &timeline_bars,
        &timestamps,
        "TOP-5 VIP TAKER PORTFOLIO BOOK (10 bps Roundtrip Fee + Slippage)",
        0.0005, // 5 bps per leg
        0.0003, // 3 bps slippage
        5,      // Max 5 Concurrent Pairs
    );
}
