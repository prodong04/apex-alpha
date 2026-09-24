use anyhow::Result;
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::Deserialize;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

const SYMBOLS: &[&str] = &[
    "BTCUSDT", "ETHUSDT", "SOLUSDT", "AVAXUSDT", "SUIUSDT",
    "NEARUSDT", "APTUSDT", "ARBUSDT", "OPUSDT", "LINKUSDT",
    "DOGEUSDT", "PEPEUSDT", "RENDERUSDT", "FETUSDT", "TIAUSDT",
    "INJUSDT", "FTMUSDT", "BNBUSDT", "XRPUSDT", "ADAUSDT",
];

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawKlineItem {
    Int(i64),
    Str(String),
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let data_dir = "/Users/elias/apex-alpha/data/klines";
    fs::create_dir_all(data_dir)?;

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()?;

    println!("🚀 Starting Multi-Asset 1-Minute Kline Historical Data Fetcher...");
    println!("• Target Symbols: {} tokens", SYMBOLS.len());
    println!("• Timeframe: 1m (1-minute candles)");
    println!("• Target History: Past 3 Days (~4,320 bars per asset)");
    println!("------------------------------------------------------------------");

    let limit = 1500; // Binance max per request
    let mut total_bars_fetched = 0;

    for symbol in SYMBOLS {
        print!("Fetching {:<10} ... ", symbol);
        std::io::stdout().flush().unwrap();

        let mut all_klines: Vec<(i64, f64, f64, f64, f64, f64)> = Vec::new(); // (ts, o, h, l, c, v)
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

            let resp = client.get(&url).send().await;
            match resp {
                Ok(res) => {
                    if let Ok(raw_rows) = res.json::<Vec<Vec<serde_json::Value>>>().await {
                        if raw_rows.is_empty() {
                            break;
                        }

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
                Err(e) => {
                    eprintln!("Error fetching {}: {:?}", symbol, e);
                    break;
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
        }

        // Deduplicate & Sort by timestamp
        all_klines.sort_by_key(|k| k.0);
        all_klines.dedup_by_key(|k| k.0);

        let out_file_path = format!("{}/{}_1m.csv", data_dir, symbol);
        let mut f = File::create(&out_file_path)?;
        writeln!(f, "timestamp,datetime,open,high,low,close,volume")?;

        for (ts, o, h, l, c, v) in &all_klines {
            let dt = DateTime::<Utc>::from_timestamp_millis(*ts)
                .map(|d| d.to_rfc3339())
                .unwrap_or_default();
            writeln!(f, "{},{},{:.6},{:.6},{:.6},{:.6},{:.6}", ts, dt, o, h, l, c, v)?;
        }

        println!("✅ Saved {:>5} bars to {}", all_klines.len(), out_file_path);
        total_bars_fetched += all_klines.len();
    }

    println!("------------------------------------------------------------------");
    println!("🎉 All {} symbols fetched! Total {} 1m bars stored.", SYMBOLS.len(), total_bars_fetched);
    Ok(())
}
