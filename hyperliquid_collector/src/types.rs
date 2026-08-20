use serde::{Deserialize, Serialize};

/// Hyperliquid WebSocket subscription request
#[derive(Debug, Serialize, Deserialize)]
pub struct WsSubscriptionRequest {
    pub method: String,
    pub subscription: SubscriptionType,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SubscriptionType {
    #[serde(rename = "l2Book")]
    L2Book { coin: String },
    #[serde(rename = "trades")]
    Trades { coin: String },
    #[serde(rename = "activeAssetCtx")]
    ActiveAssetCtx { coin: String },
    #[serde(rename = "allMids")]
    AllMids,
}

/// A single level in the orderbook (Price, Size, Number of orders)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookLevel {
    pub px: String,
    pub sz: String,
    pub n: u32,
}

/// L2 Orderbook data payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L2BookData {
    pub coin: String,
    pub time: u64,
    pub levels: (Vec<BookLevel>, Vec<BookLevel>),
}

/// Real-time individual trade (Tick data)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub coin: String,
    pub side: String, // "B" for Buy, "A" for Ask/Sell
    pub px: String,
    pub sz: String,
    pub time: u64,
    #[serde(default)]
    pub hash: String,
    #[serde(default)]
    pub tid: u64,
}

/// Asset Context (Funding, Open Interest, Oracle Price, Impact Prices)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetContext {
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
    #[serde(rename = "prevDayPx", default)]
    pub prev_day_px: Option<String>,
}

/// Active Asset Context wrapper payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveAssetCtxData {
    pub coin: String,
    pub ctx: AssetContext,
}

/// General inbound WebSocket message from Hyperliquid
#[derive(Debug, Deserialize)]
#[serde(tag = "channel")]
pub enum WsMessage {
    #[serde(rename = "l2Book")]
    L2Book { data: L2BookData },
    #[serde(rename = "trades")]
    Trades { data: Vec<Trade> },
    #[serde(rename = "activeAssetCtx")]
    ActiveAssetCtx { data: ActiveAssetCtxData },
    #[serde(rename = "subscriptionResponse")]
    SubscriptionResponse { data: serde_json::Value },
    #[serde(rename = "pong")]
    Pong,
    #[serde(other)]
    Unknown,
}
