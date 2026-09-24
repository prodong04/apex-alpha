use anyhow::Result;
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde_json::Value;
use std::fs::{self, File};
use std::io::Write;

#[tokio::main]
async fn main() -> Result<()> {
    let data_dir = "/Users/elias/apex-alpha/data/klines_expanded";
    fs::create_dir_all(data_dir)?;

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()?;

    println!("🚀 Fetching Top 60 Liquid USDT-M Perpetual Futures Symbols...");

    // 1. Get 24hr ticker data to sort by volume
    let ticker_url = "https://fapi.binance.com/fapi/v1/ticker/24hr";
    let resp = client.get(ticker_url).send().await?.json::<Vec<Value>>().await?;

    let mut tickers: Vec<(String, f64)> = Vec::new();
    for item in resp {
        if let (Some(sym), Some(vol_str)) = (item["symbol"].as_str(), item["quoteVolume"].as_str()) {
            if sym.ends_with("USDT") && !sym.starts_with("USDC") && !sym.contains("1000") && !sym.contains("BTCDOM") {
                let vol = vol_str.parse::<f64>().unwrap_or(0.0);
                tickers.push((sym.to_string(), vol));
            }
        }
    }

    // Sort by 24h quote volume descending
    tickers.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    let target_symbols: Vec<String> = tickers.into_iter().take(60).map(|(s, _)| s).collect();

    println!("✅ Selected Top {} USDT-M Futures by 24h Volume:", target_symbols.len());
    for (idx, chunk) in target_symbols.chunks(10).enumerate() {
        println!("  [{:>2}..{:>2}]: {:?}", idx * 10 + 1, idx * 10 + chunk.len(), chunk);
    }
    println!("------------------------------------------------------------------");

    let limit = 1500;
    let mut total_downloaded = 0;

    for (i, symbol) in target_symbols.iter().enumerate() {
        print!("({:>2}/{}) Fetching {:<12} ... ", i + 1, target_symbols.len(), symbol);
        std::io::stdout().flush().unwrap();

        let mut all_klines: Vec<(i64, f64, f64, f64, f64, f64)> = Vec::new();
        let mut end_time: Option<i64> = None;

        // Fetch 3 chunks of 1500 bars -> ~4500 1m bars (~3.1 days)
        for _ in 0..3 {
            let mut url = format!(
                "https://fapi.binance.com/fapi/v1/klines?symbol={}&interval=1m&limit={}",
                symbol, limit
            );
            if let Some(et) = end_time {
                url.push_str(&format!("&endTime={}", et));
            }

            if let Ok(res) = client.get(&url).send().await {
                if let Ok(raw_rows) = res.json::<Vec<Vec<Value>>>().await {
                    if raw_rows.is_empty() { break; }
                    let mut chunk = Vec::new();
                    for row in raw_rows {
                        if row.len() >= 6 {
                            let ts = row[0].as_i64().unwrap_or(0);
                            let open = row[1].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0);
                            let high = row[2].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0);
                            let low = row[3].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0);
                            let close = row[4].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0);
                            let vol = row[5].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0);
                            chunk.push((ts, open, high, low, close, vol));
                        }
                    }
                    if let Some(first) = chunk.first() {
                        end_time = Some(first.0 - 1);
                    }
                    chunk.append(&mut all_klines);
                    all_klines = chunk;
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        all_klines.sort_by_key(|k| k.0);
        all_klines.dedup_by_key(|k| k.0);

        if all_klines.len() > 1000 {
            let out_path = format!("{}/{}_1m.csv", data_dir, symbol);
            let mut f = File::create(&out_path)?;
            writeln!(f, "timestamp,datetime,open,high,low,close,volume")?;
            for (ts, o, h, l, c, v) in &all_klines {
                let dt = DateTime::<Utc>::from_timestamp_millis(*ts)
                    .map(|d| d.to_rfc3339())
                    .unwrap_or_default();
                writeln!(f, "{},{},{:.6},{:.6},{:.6},{:.6},{:.6}", ts, dt, o, h, l, c, v)?;
            }
            println!("✅ Saved {:>5} bars", all_klines.len());
            total_downloaded += all_klines.len();
        } else {
            println!("⚠️ Skipped (only {} bars)", all_klines.len());
        }
    }

    println!("------------------------------------------------------------------");
    println!("🎉 Download Complete! Stored {} total 1m bars across expanded universe.", total_downloaded);
    Ok(())
}
