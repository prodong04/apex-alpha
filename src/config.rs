use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub symbol: String,
    pub data_dir: String,
    pub live_mode: bool,
    pub initial_balance: f64,
    #[serde(default = "default_leverage")]
    pub leverage: f64,
    pub maker_fee_rate: f64,
    pub taker_fee_rate: f64,
    pub enable_binance_spot: bool,
    pub enable_binance_futures: bool,
    pub enable_hyperliquid: bool,
    pub server_host: String,
    pub server_port: u16,
    #[serde(default = "default_strategy")]
    pub strategy: String,
}

fn default_leverage() -> f64 {
    1.0
}

fn default_strategy() -> String {
    "passive".to_string()
}

impl AppConfig {
    pub fn load_or_default<P: AsRef<Path>>(path: P) -> Self {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(config) = toml::from_str::<AppConfig>(&content) {
                return config;
            }
        }
        
        // Fallback default
        AppConfig {
            symbol: "BTC".to_string(),
            data_dir: "data/live".to_string(),
            live_mode: false,
            initial_balance: 10000.0,
            leverage: 1.0,
            maker_fee_rate: 0.0002,
            taker_fee_rate: 0.0005,
            enable_binance_spot: true,
            enable_binance_futures: true,
            enable_hyperliquid: true,
            server_host: "0.0.0.0".to_string(),
            server_port: 3000,
            strategy: "passive".to_string(),
        }
    }
}
