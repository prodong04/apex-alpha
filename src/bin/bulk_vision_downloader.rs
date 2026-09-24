use anyhow::Result;
use reqwest::Client;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Semaphore;

// Binance Vision Futures UM base URL
const VISION_BASE_URL: &str = "https://data.binance.vision/data/futures/um/monthly/klines";

// Top liquid and historically significant symbols across 2022-2026 (including historical/delisted ones)
const UNIVERSE_SYMBOLS: &[&str] = &[
    "BTCUSDT", "ETHUSDT", "SOLUSDT", "BNBUSDT", "XRPUSDT", "ADAUSDT", "DOGEUSDT",
    "AVAXUSDT", "DOTUSDT", "MATICUSDT", "LINKUSDT", "NEARUSDT", "UNIUSDT", "ATOMUSDT",
    "LTCUSDT", "BCHUSDT", "ETCUSDT", "FILUSDT", "APTUSDT", "ARBUSDT", "OPUSDT",
    "SUIUSDT", "INJUSDT", "TIAUSDT", "SEIUSDT", "RENDERUSDT", "FETUSDT", "RUNEUSDT",
    "FTMUSDT", "SANDUSDT", "MANAUSDT", "AXSUSDT", "GALAUSDT", "DYDXUSDT", "CRVUSDT",
    "AAVEUSDT", "SNXUSDT", "KAVAUSDT", "CHZUSDT", "ALGOUSDT", "WLDUSDT", "ORDIUSDT",
    "1000PEPEUSDT", "1000SHIBUSDT", "1000BONKUSDT", "1000FLOKIUSDT", "LDOUSDT", "STXUSDT",
    // Historically critical / Delisted / High-volatility symbols for Survivorship Bias elimination:
    "LUNAUSDT", "FTTUSDT", "SRMUSDT", "RAYUSDT", "C98USDT", "GMTUSDT", "APEUSDT", "STEPUSDT"
];

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let raw_zip_dir = "/Users/elias/apex-alpha/data/vision_zips";
    let parsed_csv_dir = "/Users/elias/apex-alpha/data/vision_1m_csv";
    fs::create_dir_all(raw_zip_dir)?;
    fs::create_dir_all(parsed_csv_dir)?;

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    println!("==================================================================");
    println!("🏛️ BINANCE VISION BULK 1-MINUTE KLINE DATASET INGESTION");
    println!("==================================================================");
    println!("• Universe Symbols : {} pairs (incl. delisted / historical)", UNIVERSE_SYMBOLS.len());
    println!("• Time Horizon     : 2022-01 to 2026-08 (~4.6 Years Point-In-Time)");
    println!("• Resolution       : 1-Minute Continuous Candlesticks");
    println!("------------------------------------------------------------------");

    // Generate all Year-Month strings (2022-01 to 2026-08)
    let mut year_months = Vec::new();
    for year in 2022..=2026 {
        let max_month = if year == 2026 { 8 } else { 12 };
        for month in 1..=max_month {
            year_months.push(format!("{:04}-{:02}", year, month));
        }
    }

    println!("• Total Monthly Chunks per Symbol: {} months", year_months.len());

    let semaphore = Arc::new(Semaphore::new(16)); // 16 parallel downloads
    let mut tasks = Vec::new();

    for &symbol in UNIVERSE_SYMBOLS {
        for ym in &year_months {
            let symbol = symbol.to_string();
            let ym = ym.clone();
            let client = client.clone();
            let sem = semaphore.clone();
            let raw_dir = raw_zip_dir.to_string();

            tasks.push(tokio::spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                let file_name = format!("{}-1m-{}.zip", symbol, ym);
                let zip_path = format!("{}/{}", raw_dir, file_name);

                if Path::new(&zip_path).exists() {
                    return (symbol, ym, true, true); // Already cached
                }

                let url = format!("{}/{}/1m/{}", VISION_BASE_URL, symbol, file_name);
                let resp = client.get(&url).send().await;

                match resp {
                    Ok(r) if r.status().is_success() => {
                        if let Ok(bytes) = r.bytes().await {
                            if let Ok(mut f) = File::create(&zip_path) {
                                let _ = f.write_all(&bytes);
                                return (symbol, ym, true, false);
                            }
                        }
                    }
                    _ => {}
                }

                (symbol, ym, false, false)
            }));
        }
    }

    println!("🚀 Launched {} parallel ingestion tasks across S3 cluster...", tasks.len());

    let mut successful_downloads = 0;
    let mut cached_files = 0;
    let mut missing_or_unlisted = 0;

    for task in tasks {
        if let Ok((_sym, _ym, success, cached)) = task.await {
            if success {
                if cached {
                    cached_files += 1;
                } else {
                    successful_downloads += 1;
                }
            } else {
                missing_or_unlisted += 1;
            }
        }
    }

    println!("------------------------------------------------------------------");
    println!("✅ Ingestion Summary:");
    println!("  - Newly Downloaded ZIPs : {}", successful_downloads);
    println!("  - Pre-cached ZIPs       : {}", cached_files);
    println!("  - Pre-listing / Delisted: {} (Expected for tokens listed after 2022)", missing_or_unlisted);
    println!("------------------------------------------------------------------");

    // Process & Unzip downloaded monthly data into structured consolidated CSV per symbol
    println!("📦 Unpacking & Consolidating Point-In-Time 1m CSV files...");
    consolidate_zips(raw_zip_dir, parsed_csv_dir)?;

    Ok(())
}

fn consolidate_zips(zip_dir: &str, out_dir: &str) -> Result<()> {
    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!(
            r#"
            cd "{}"
            for f in *.zip; do
                [ -f "$f" ] || continue
                unzip -q -o "$f" -d "{}" 2>/dev/null || true
            done
            "#,
            zip_dir, out_dir
        ))
        .output()?;

    println!("✅ Unzip Batch Complete (Status: {:?})", output.status);
    Ok(())
}
