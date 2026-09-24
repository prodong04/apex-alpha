use chrono::Timelike;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader};

fn main() {
    let file = File::open("/Users/elias/apex-alpha/scratch/backtest_pnl_timeseries.csv").unwrap();
    let reader = BufReader::new(file);

    let mut hourly_pnl: BTreeMap<u32, (usize, usize, f64, f64)> = BTreeMap::new();

    for i in 0..24 {
        hourly_pnl.insert(i, (0, 0, 0.0, 0.0));
    }

    for (idx, line) in reader.lines().enumerate() {
        if idx == 0 { continue; }
        if let Ok(l) = line {
            let parts: Vec<&str> = l.split(',').collect();
            if parts.len() >= 10 {
                let ts: i64 = parts[0].parse().unwrap_or(0);
                let gross: f64 = parts[6].parse().unwrap_or(0.0);
                let net_m: f64 = parts[8].parse().unwrap_or(0.0);

                if let Some(dt) = chrono::DateTime::from_timestamp_millis(ts) {
                    let hour_kst = (dt.hour() + 9) % 24;
                    let entry = hourly_pnl.entry(hour_kst).or_insert((0, 0, 0.0, 0.0));
                    entry.0 += 1;
                    if net_m > 0.0 { entry.1 += 1; }
                    entry.2 += gross;
                    entry.3 += net_m;
                }
            }
        }
    }

    println!("| KST 시간대 | 거래수 | 승률 (%) | 총이익 ($) | 메이커 순익 ($) | 글로벌 세션 특징 |");
    println!("| :---: | :---: | :---: | :---: | :---: | :--- |");
    for (hour, (trades, wins, gross, net_m)) in hourly_pnl {
        let wr = if trades > 0 { (wins as f64 / trades as f64) * 100.0 } else { 0.0 };
        let session = match hour {
            9..=16 => "🌏 아시아 세션 (한국/도쿄)",
            17..=21 => "🌍 유럽 개장 세션 (런던)",
            22..=23 | 0..=3 => "🇺🇸 미국 본장 세션 (뉴욕 피크)",
            _ => "🌙 심야 저변동성 세션",
        };
        println!("| {:02}시 ~ {:02}시 | {:>3}회 | {:>6.1}% | ${:>8.2} | ${:>9.2} | {} |", hour, (hour+1)%24, trades, wr, gross, net_m, session);
    }
}
