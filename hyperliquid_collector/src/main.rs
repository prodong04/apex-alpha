mod binance_futures;
mod binance_spot;
mod hyperliquid;
mod types;

use anyhow::Result;
use std::env;
use std::fs::create_dir_all;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{interval, sleep};
use tracing::info;

const DEFAULT_COIN: &str = "BTC";
const DEFAULT_DATA_DIR: &str = "data/live";

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Initialize structured logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let target_coin = env::var("TARGET_COIN").unwrap_or_else(|_| DEFAULT_COIN.to_string());
    let data_dir = env::var("DATA_DIR").unwrap_or_else(|_| DEFAULT_DATA_DIR.to_string());

    let enable_spot = env::var("ENABLE_BINANCE_SPOT").map(|v| v != "false" && v != "0").unwrap_or(true);
    let enable_futures = env::var("ENABLE_BINANCE_FUTURES").map(|v| v != "false" && v != "0").unwrap_or(true);
    let enable_hl = env::var("ENABLE_HYPERLIQUID").map(|v| v != "false" && v != "0").unwrap_or(true);

    info!("==================================================================");
    info!("🚀 Unified Multi-Exchange Quant Market Data Collector");
    info!("🎯 Target Symbol      : {}", target_coin);
    info!("📁 Output Directory    : {}", data_dir);
    info!("⚡ Binance Spot        : {}", if enable_spot { "ENABLED (100ms L2 + BBO + AggTrades)" } else { "DISABLED" });
    info!("⚡ Binance Futures     : {}", if enable_futures { "ENABLED (100ms L2 + BBO + AggTrades + Mark/Funding + Liqs + Metrics)" } else { "DISABLED" });
    info!("⚡ Hyperliquid         : {}", if enable_hl { "ENABLED (L2 + Trades + AssetContext)" } else { "DISABLED" });
    info!("==================================================================");

    create_dir_all(&data_dir)?;

    let is_running = Arc::new(AtomicBool::new(true));
    let is_running_clone = is_running.clone();

    // 2. Setup graceful shutdown for Docker / AWS SIGTERM / SIGINT
    tokio::spawn(async move {
        if let Ok(()) = tokio::signal::ctrl_c().await {
            info!("🛑 Received termination signal (SIGINT/SIGTERM). Shutting down cleanly...");
            is_running_clone.store(false, Ordering::SeqCst);
        }
    });

    // Atomic Counters for Unified Status Reporting
    let spot_depth = Arc::new(AtomicU64::new(0));
    let spot_bbo = Arc::new(AtomicU64::new(0));
    let spot_trades = Arc::new(AtomicU64::new(0));

    let fut_depth = Arc::new(AtomicU64::new(0));
    let fut_bbo = Arc::new(AtomicU64::new(0));
    let fut_trades = Arc::new(AtomicU64::new(0));
    let fut_mark = Arc::new(AtomicU64::new(0));
    let fut_liqs = Arc::new(AtomicU64::new(0));
    let fut_metrics = Arc::new(AtomicU64::new(0));

    let hl_depth = Arc::new(AtomicU64::new(0));
    let hl_trades = Arc::new(AtomicU64::new(0));
    let hl_metrics = Arc::new(AtomicU64::new(0));

    // 3. Spawn Worker Tasks
    let mut task_handles = Vec::new();

    if enable_spot {
        let coin = target_coin.clone();
        let dir = data_dir.clone();
        let sd = spot_depth.clone();
        let sb = spot_bbo.clone();
        let st = spot_trades.clone();
        let run = is_running.clone();
        task_handles.push(tokio::spawn(async move {
            binance_spot::run_binance_spot_loop(coin, dir, sd, sb, st, run).await;
        }));
    }

    if enable_futures {
        let coin = target_coin.clone();
        let dir = data_dir.clone();
        let fd = fut_depth.clone();
        let fb = fut_bbo.clone();
        let ft = fut_trades.clone();
        let fm = fut_mark.clone();
        let fl = fut_liqs.clone();
        let fmet = fut_metrics.clone();
        let run = is_running.clone();
        task_handles.push(tokio::spawn(async move {
            binance_futures::run_binance_futures_loop(coin, dir, fd, fb, ft, fm, fl, fmet, run).await;
        }));
    }

    if enable_hl {
        let coin = target_coin.clone();
        let dir = data_dir.clone();
        let ht = hl_trades.clone();
        let hd = hl_depth.clone();
        let hm = hl_metrics.clone();
        let run = is_running.clone();
        task_handles.push(tokio::spawn(async move {
            hyperliquid::run_hyperliquid_loop(coin, dir, ht, hd, hm, run).await;
        }));
    }

    // 4. Unified Periodic Status Logger
    let is_running_logger = is_running.clone();
    let coin_display = target_coin.clone();
    tokio::spawn(async move {
        let mut interval_ticker = interval(Duration::from_secs(3));
        // Skip first tick
        interval_ticker.tick().await;

        while is_running_logger.load(Ordering::SeqCst) {
            interval_ticker.tick().await;
            if !is_running_logger.load(Ordering::SeqCst) {
                break;
            }

            info!(
                "⚡ [LIVE {}] 🟡 Spot: Books {} | BBO {} | Trades {} | 🟢 Futures: Books {} | BBO {} | Trades {} | Mark {} | Liqs {} | 🔵 HL: Books {} | Trades {}",
                coin_display,
                spot_depth.load(Ordering::Relaxed),
                spot_bbo.load(Ordering::Relaxed),
                spot_trades.load(Ordering::Relaxed),
                fut_depth.load(Ordering::Relaxed),
                fut_bbo.load(Ordering::Relaxed),
                fut_trades.load(Ordering::Relaxed),
                fut_mark.load(Ordering::Relaxed),
                fut_liqs.load(Ordering::Relaxed),
                hl_depth.load(Ordering::Relaxed),
                hl_trades.load(Ordering::Relaxed)
            );
        }
    });

    // Wait until running flag is false
    while is_running.load(Ordering::SeqCst) {
        sleep(Duration::from_millis(500)).await;
    }

    info!("⏳ Waiting for collector worker tasks to gracefully shutdown and flush...");
    for handle in task_handles {
        let _ = handle.await;
    }

    info!("🏁 All market collector workers exited cleanly with buffers flushed. Goodbye!");
    Ok(())
}
