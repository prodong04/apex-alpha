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

/// General inbound WebSocket message from Hyperliquid
#[derive(Debug, Deserialize)]
#[serde(tag = "channel")]
pub enum WsMessage {
    #[serde(rename = "l2Book")]
    L2Book { data: L2BookData },
    #[serde(rename = "trades")]
    Trades { data: Vec<Trade> },
    #[serde(rename = "subscriptionResponse")]
    SubscriptionResponse { data: serde_json::Value },
    #[serde(rename = "pong")]
    Pong,
    #[serde(other)]
    Unknown,
}
