use std::fs::File;
use std::io::{BufRead, BufReader};

fn main() {
    let file = File::open("/Users/elias/apex-alpha/scratch/backtest_pnl_timeseries.csv").unwrap();
    let reader = BufReader::new(file);

    let mut net_pnls_maker = Vec::new();
    let mut net_pnls_taker = Vec::new();

    for (idx, line) in reader.lines().enumerate() {
        if idx == 0 { continue; }
        if let Ok(l) = line {
            let parts: Vec<&str> = l.split(',').collect();
            if parts.len() >= 13 {
                let m: f64 = parts[8].parse().unwrap_or(0.0);
                let t: f64 = parts[11].parse().unwrap_or(0.0);
                net_pnls_maker.push(m);
                net_pnls_taker.push(t);
            }
        }
    }

    let n = net_pnls_maker.len() as f64;
    let sum_m: f64 = net_pnls_maker.iter().sum();
    let mean_m = sum_m / n;
    let var_m = net_pnls_maker.iter().map(|p| (p - mean_m).powi(2)).sum::<f64>() / (n - 1.0);
    let std_m = var_m.sqrt();

    let downside_var_m = net_pnls_maker.iter().map(|p| if *p < 0.0 { p.powi(2) } else { 0.0 }).sum::<f64>() / (n - 1.0);
    let downside_std_m = downside_var_m.sqrt();

    // 62.5 hours dataset ➔ Annualization factor (365 * 24 = 8,760 hours / year)
    let trades_per_year = n * (8760.0 / 62.5);
    let sharpe_trade_m = mean_m / std_m;
    let sharpe_annual_m = sharpe_trade_m * trades_per_year.sqrt();
    let sortino_annual_m = (mean_m / downside_std_m) * trades_per_year.sqrt();

    // Taker calculations
    let sum_t: f64 = net_pnls_taker.iter().sum();
    let mean_t = sum_t / n;
    let var_t = net_pnls_taker.iter().map(|p| (p - mean_t).powi(2)).sum::<f64>() / (n - 1.0);
    let std_t = var_t.sqrt();
    let sharpe_trade_t = mean_t / std_t;
    let sharpe_annual_t = sharpe_trade_t * trades_per_year.sqrt();

    println!("==================================================================");
    println!("📊 SHARPE & SORTINO RATIO QUANT AUDIT (62.5H FULL DATASET)");
    println!("==================================================================");
    println!("• Total Analyzed Trades   : {}", n);
    println!("• Estimated Trades / Year : {:.0} trades/year", trades_per_year);
    println!("------------------------------------------------------------------");
    println!("🟢 1) 지정가 메이커 (MAKER 0.01% FEE):");
    println!("  - 건당 평균 순익 (Mean) : +${:.2}", mean_m);
    println!("  - 건당 표준편차 (StdDev): ${:.2}", std_m);
    println!("  - 건당 샤프 (Trade SR) : {:.4}", sharpe_trade_m);
    println!("  - 🔥 연간화 샤프 (Annualized Sharpe) : {:.2}", sharpe_annual_m);
    println!("  - 🔥 연간화 소르티노 (Annualized Sortino): {:.2}", sortino_annual_m);
    println!("------------------------------------------------------------------");
    println!("🔴 2) 시장가 테이커 (TAKER 0.035% FEE):");
    println!("  - 건당 평균 순익 (Mean) : -${:.2}", mean_t);
    println!("  - 건당 표준편차 (StdDev): ${:.2}", std_t);
    println!("  - 연간화 샤프 (Annualized Sharpe) : {:.2} (적자 전략)", sharpe_annual_t);
    println!("==================================================================");
}
