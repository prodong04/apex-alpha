use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use reqwest::Client;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::{interval, sleep, timeout};
use tokio_tungstenite::connect_async;
use tracing::{error, info, warn};

use crate::collector::types::{
    BinanceAggTrade, BinanceBookTicker, BinanceFuturesDepth20, BinanceMarkPrice, BinanceOpenInterest,
};
use crate::engine::types::UiEvent;
use crate::server::UiBroadcaster;

const FUTURES_WS_URL: &str = "wss://fstream.binance.com/ws";
const READ_TIMEOUT_SECS: u64 = 30;
const REST_BASE_URL: &str = "https://fapi.binance.com";

pub async fn run_binance_futures_loop(
    coin: String,
    total_depth: Arc<AtomicU64>,
    total_bbo: Arc<AtomicU64>,
    total_trades: Arc<AtomicU64>,
    total_mark: Arc<AtomicU64>,
    total_liqs: Arc<AtomicU64>,
    total_metrics: Arc<AtomicU64>,
    broadcaster: Option<UiBroadcaster>,
    is_running: Arc<AtomicBool>,
) {
    // 1. REST Metrics Poller Task (OI)
    let coin_for_rest = coin.clone();
    let is_running_rest = is_running.clone();
    let total_metrics_clone = total_metrics.clone();
    tokio::spawn(async move {
        run_futures_rest_poller(coin_for_rest, total_metrics_clone, is_running_rest).await;
    });

    // 2. High-Performance WebSocket Stream Loop
    let mut backoff_secs = 1;
    while is_running.load(Ordering::SeqCst) {
        info!("🔌 [Binance Futures] Connecting WebSocket for {}...", coin);
        let start_time = Instant::now();

        if let Err(e) = run_futures_ws_collector(
            &coin,
            total_depth.clone(),
            total_bbo.clone(),
            total_trades.clone(),
            total_mark.clone(),
            total_liqs.clone(),
            broadcaster.clone(),
            is_running.clone(),
        )
        .await
        {
            if !is_running.load(Ordering::SeqCst) {
                break;
            }
            error!("❌ [Binance Futures] Stream error: {:#}", e);
        }

        if start_time.elapsed() > Duration::from_secs(60) {
            backoff_secs = 1;
        } else {
            backoff_secs = (backoff_secs * 2).min(30);
        }

        if is_running.load(Ordering::SeqCst) {
            warn!("⏳ [Binance Futures] Reconnecting in {}s...", backoff_secs);
            sleep(Duration::from_secs(backoff_secs)).await;
        }
    }
    info!("🏁 [Binance Futures] Collector stopped.");
}

async fn run_futures_ws_collector(
    coin: &str,
    total_depth: Arc<AtomicU64>,
    total_bbo: Arc<AtomicU64>,
    total_trades: Arc<AtomicU64>,
    total_mark: Arc<AtomicU64>,
    _total_liqs: Arc<AtomicU64>,
    broadcaster: Option<UiBroadcaster>,
    is_running: Arc<AtomicBool>,
) -> Result<()> {
    let coin_lower = coin.to_lowercase();
    let symbol = format!("{}usdt", coin_lower);

    let (ws_stream, response) = connect_async(FUTURES_WS_URL)
        .await
        .with_context(|| format!("Failed to connect to Binance Futures WS: {}", FUTURES_WS_URL))?;

    info!("⚡ [Binance Futures] Connected! HTTP Status: {}", response.status());
    let (mut write, mut read) = ws_stream.split();

    // Subscribe to: aggTrade, markPrice@1s, bookTicker, depth20@100ms
    let sub_payload = serde_json::json!({
        "method": "SUBSCRIBE",
        "params": [
            format!("{}@aggTrade", symbol),
            format!("{}@markPrice@1s", symbol),
            format!("{}@bookTicker", symbol),
            format!("{}@depth20@100ms", symbol),
        ],
        "id": 101
    });

    write
        .send(tokio_tungstenite::tungstenite::protocol::Message::Text(sub_payload.to_string()))
        .await?;
    info!("✅ [Binance Futures] Subscriptions sent for {} (AggTrades, MarkPrice, BBO, Depth20)", symbol);

    while is_running.load(Ordering::SeqCst) {
        match timeout(Duration::from_secs(READ_TIMEOUT_SECS), read.next()).await {
            Ok(Some(Ok(msg))) => {
                if let tokio_tungstenite::tungstenite::protocol::Message::Text(text) = msg {
                    if text.contains("\"e\":\"aggTrade\"") {
                        if let Ok(data) = serde_json::from_str::<BinanceAggTrade>(&text) {
                            total_trades.fetch_add(1, Ordering::Relaxed);
                            let p: f64 = data.price.parse().unwrap_or(0.0);
                            let q: f64 = data.quantity.parse().unwrap_or(0.0);
                            let side_str = if data.is_buyer_maker { "SELL" } else { "BUY" };

                            if let Some(ref bc) = broadcaster {
                                bc.send(UiEvent::ExchangeTrade {
                                    timestamp: data.trade_time as i64,
                                    exchange: "Binance Futures".to_string(),
                                    symbol: coin.to_string(),
                                    side: side_str.to_string(),
                                    price: p,
                                    qty: q,
                                });
                            }
                        }
                    } else if text.contains("\"e\":\"markPriceUpdate\"") {
                        if serde_json::from_str::<BinanceMarkPrice>(&text).is_ok() {
                            total_mark.fetch_add(1, Ordering::Relaxed);
                        }
                    } else if text.contains("\"e\":\"bookTicker\"") || (text.contains("\"u\":") && text.contains("\"b\":") && text.contains("\"a\":")) {
                        if serde_json::from_str::<BinanceBookTicker>(&text).is_ok() {
                            total_bbo.fetch_add(1, Ordering::Relaxed);
                        }
                    } else if text.contains("\"e\":\"depthUpdate\"") || text.contains("\"lastUpdateId\"") {
                        if serde_json::from_str::<BinanceFuturesDepth20>(&text).is_ok() {
                            total_depth.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
            }
            Ok(Some(Err(e))) => {
                error!("[Binance Futures] WS read error: {}", e);
                break;
            }
            Ok(None) => {
                warn!("[Binance Futures] Stream closed by server.");
                break;
            }
            Err(_) => {
                warn!("[Binance Futures] WS timeout ({}s), reconnecting...", READ_TIMEOUT_SECS);
                break;
            }
        }
    }

    Ok(())
}

async fn run_futures_rest_poller(
    coin: String,
    total_metrics: Arc<AtomicU64>,
    is_running: Arc<AtomicBool>,
) {
    let client = Client::builder().timeout(Duration::from_secs(5)).build().unwrap_or_default();
    let symbol = format!("{}USDT", coin.to_uppercase());
    let mut poll_ticker = interval(Duration::from_secs(1));

    while is_running.load(Ordering::SeqCst) {
        poll_ticker.tick().await;
        if !is_running.load(Ordering::SeqCst) {
            break;
        }

        let oi_url = format!("{}/fapi/v1/openInterest?symbol={}", REST_BASE_URL, symbol);
        if let Ok(resp) = client.get(&oi_url).send().await {
            if resp.json::<BinanceOpenInterest>().await.is_ok() {
                total_metrics.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
    info!("🏁 [Binance Futures REST] Poller stopped.");
}
