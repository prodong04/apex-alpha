#![allow(dead_code)]
use serde::{Deserialize, Serialize};

// ============================================================================
// 1. Hyperliquid Schemas
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct HlWsSubscriptionRequest {
    pub method: String,
    pub subscription: HlSubscriptionType,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum HlSubscriptionType {
    #[serde(rename = "l2Book")]
    L2Book { coin: String },
    #[serde(rename = "trades")]
    Trades { coin: String },
    #[serde(rename = "activeAssetCtx")]
    ActiveAssetCtx { coin: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HlBookLevel {
    pub px: String,
    pub sz: String,
    pub n: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HlL2BookData {
    pub coin: String,
    pub time: u64,
    pub levels: (Vec<HlBookLevel>, Vec<HlBookLevel>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HlTrade {
    pub coin: String,
    pub side: String,
    pub px: String,
    pub sz: String,
    pub time: u64,
    #[serde(default)]
    pub hash: String,
    #[serde(default)]
    pub tid: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HlAssetContext {
    pub funding: String,
    #[serde(rename = "openInterest")]
    pub open_interest: String,
    #[serde(rename = "oraclePx")]
    pub oracle_px: String,
    #[serde(rename = "markPx", default)]
    pub mark_px: String,
    #[serde(rename = "midPx", default)]
    pub mid_px: Option<String>,
    #[serde(rename = "dayNtlVlm", default)]
    pub day_ntl_vlm: String,
    #[serde(rename = "impactPxs", default)]
    pub impact_pxs: Option<Vec<String>>,
    #[serde(default)]
    pub premium: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HlActiveAssetCtxData {
    pub coin: String,
    pub ctx: HlAssetContext,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "channel")]
pub enum HlWsMessage {
    #[serde(rename = "l2Book")]
    L2Book { data: HlL2BookData },
    #[serde(rename = "trades")]
    Trades { data: Vec<HlTrade> },
    #[serde(rename = "activeAssetCtx")]
    ActiveAssetCtx { data: HlActiveAssetCtxData },
    #[serde(rename = "subscriptionResponse")]
    SubscriptionResponse { data: serde_json::Value },
    #[serde(rename = "pong")]
    Pong,
    #[serde(other)]
    Unknown,
}

// ============================================================================
// 2. Binance Spot & Futures Schemas (Combined Stream Wrapper)
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct BinanceCombinedStream<T> {
    pub stream: String,
    pub data: T,
}

#[derive(Debug, Deserialize)]
pub struct BinanceRawStream {
    pub stream: String,
    pub data: serde_json::Value,
}

// Spot / Futures BBO (bookTicker)
#[derive(Debug, Clone, Deserialize)]
pub struct BinanceBookTicker {
    #[serde(rename = "u")]
    pub update_id: u64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "b")]
    pub best_bid_px: String,
    #[serde(rename = "B")]
    pub best_bid_qty: String,
    #[serde(rename = "a")]
    pub best_ask_px: String,
    #[serde(rename = "A")]
    pub best_ask_qty: String,
    #[serde(rename = "E", default)]
    pub event_time: u64,
    #[serde(rename = "T", default)]
    pub transact_time: u64,
}

// Spot / Futures AggTrade
#[derive(Debug, Clone, Deserialize)]
pub struct BinanceAggTrade {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: u64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "a")]
    pub agg_trade_id: u64,
    #[serde(rename = "p")]
    pub price: String,
    #[serde(rename = "q")]
    pub quantity: String,
    #[serde(rename = "f")]
    pub first_trade_id: u64,
    #[serde(rename = "l")]
    pub last_trade_id: u64,
    #[serde(rename = "T")]
    pub trade_time: u64,
    #[serde(rename = "m")]
    pub is_buyer_maker: bool,
}

// Spot Partial Depth (depth20@100ms)
#[derive(Debug, Clone, Deserialize)]
pub struct BinanceSpotDepth20 {
    #[serde(rename = "lastUpdateId")]
    pub last_update_id: u64,
    pub bids: Vec<[String; 2]>,
    pub asks: Vec<[String; 2]>,
}

// Futures Partial Depth (depth20@100ms)
#[derive(Debug, Clone, Deserialize)]
pub struct BinanceFuturesDepth20 {
    #[serde(rename = "e", default)]
    pub event_type: String,
    #[serde(rename = "E", default)]
    pub event_time: u64,
    #[serde(rename = "T", default)]
    pub transact_time: u64,
    #[serde(rename = "s", default)]
    pub symbol: String,
    #[serde(rename = "U", default)]
    pub first_update_id: u64,
    #[serde(rename = "u", default)]
    pub last_update_id: u64,
    #[serde(rename = "pu", default)]
    pub prev_last_update_id: u64,
    #[serde(rename = "b")]
    pub bids: Vec<[String; 2]>,
    #[serde(rename = "a")]
    pub asks: Vec<[String; 2]>,
}

// Futures Mark Price & Funding (markPrice@1s)
#[derive(Debug, Clone, Deserialize)]
pub struct BinanceMarkPrice {
    #[serde(rename = "e", default)]
    pub event_type: String,
    #[serde(rename = "E", default)]
    pub event_time: u64,
    #[serde(rename = "s", default)]
    pub symbol: String,
    #[serde(rename = "p")]
    pub mark_price: String,
    #[serde(rename = "i")]
    pub index_price: String,
    #[serde(rename = "P", default)]
    pub est_settle_price: String,
    #[serde(rename = "r")]
    pub funding_rate: String,
    #[serde(rename = "T", default)]
    pub next_funding_time: u64,
}

// Futures Liquidation Order (forceOrder)
#[derive(Debug, Clone, Deserialize)]
pub struct BinanceForceOrderWrapper {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: u64,
    #[serde(rename = "o")]
    pub order: BinanceForceOrder,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BinanceForceOrder {
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "S")]
    pub side: String,
    #[serde(rename = "o")]
    pub order_type: String,
    #[serde(rename = "f")]
    pub time_in_force: String,
    #[serde(rename = "q")]
    pub orig_qty: String,
    #[serde(rename = "p")]
    pub price: String,
    #[serde(rename = "ap")]
    pub avg_price: String,
    #[serde(rename = "X")]
    pub order_status: String,
    #[serde(rename = "l")]
    pub last_filled_qty: String,
    #[serde(rename = "z")]
    pub accum_filled_qty: String,
    #[serde(rename = "T")]
    pub trade_time: u64,
}

// ============================================================================
// 3. Binance REST Metrics Schemas
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct BinanceOpenInterest {
    #[serde(rename = "openInterest")]
    pub open_interest: String,
    pub symbol: String,
    pub time: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BinanceLongShortRatio {
    pub symbol: String,
    #[serde(rename = "longShortRatio")]
    pub long_short_ratio: String,
    #[serde(rename = "longAccount")]
    pub long_account: String,
    #[serde(rename = "shortAccount")]
    pub short_account: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BinanceTakerRatio {
    #[serde(rename = "buySellRatio")]
    pub buy_sell_ratio: String,
    #[serde(rename = "buyVol")]
    pub buy_vol: String,
    #[serde(rename = "sellVol")]
    pub sell_vol: String,
    pub timestamp: u64,
}
