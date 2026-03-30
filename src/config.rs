use serde::{Deserialize, Serialize};
use std::fs;
use rust_decimal::Decimal;
use std::collections::HashMap;
use crate::model::ExchangeId;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub margin_threshold_low: Decimal,    // 0.4 (40%)
    pub margin_threshold_high: Decimal,   // 0.7 (70%)
    pub concentration_threshold: Decimal, // 0.7 (70%)
    pub target_margin_ratio: Decimal,     // 0.2 (20%)
    pub polling_interval_ms: u64,         // 5000
    pub automated_rebalance_enabled: bool,
    pub wallets: HashMap<ExchangeId, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SecretsConfig {
    pub binance_key: Option<String>,
    pub binance_secret: Option<String>,
    pub bybit_key: Option<String>,
    pub bybit_secret: Option<String>,
    pub bitget_key: Option<String>,
    pub bitget_secret: Option<String>,
    pub bitget_passphrase: Option<String>,
    pub mexc_key: Option<String>,
    pub mexc_secret: Option<String>,
    pub okx_key: Option<String>,
    pub okx_secret: Option<String>,
    pub okx_passphrase: Option<String>,
    pub telegram_token: Option<String>,
    pub telegram_chat_id: Option<String>,
}

impl SecretsConfig {
    pub fn load() -> Self {
        let path = "secrets.json";
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(config) = serde_json::from_str(&content) {
                return config;
            }
        }
        
        // Fallback to Env if JSON doesn't exist (Migration path)
        let mut s = Self::default();
        use std::env;
        s.binance_key = env::var("BINANCE_API_KEY").ok();
        s.binance_secret = env::var("BINANCE_API_SECRET").ok();
        s.bybit_key = env::var("BYBIT_API_KEY").ok();
        s.bybit_secret = env::var("BYBIT_API_SECRET").ok();
        s.bitget_key = env::var("BITGET_API_KEY").ok();
        s.bitget_secret = env::var("BITGET_API_SECRET").ok();
        s.bitget_passphrase = env::var("BITGET_API_PASSPHRASE").ok();
        s.mexc_key = env::var("MEXC_API_KEY").ok();
        s.mexc_secret = env::var("MEXC_API_SECRET").ok();
        s.okx_key = env::var("OKX_API_KEY").ok();
        s.okx_secret = env::var("OKX_API_SECRET").ok();
        s.okx_passphrase = env::var("OKX_API_PASSPHRASE").ok();
        s.telegram_token = env::var("TELEGRAM_BOT_TOKEN").ok();
        s.telegram_chat_id = env::var("TELEGRAM_CHAT_ID").ok();
        s
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let content = serde_json::to_string_pretty(self)?;
        fs::write("secrets.json", content)?;
        Ok(())
    }
}

impl AppConfig {
    pub fn load() -> Self {
        let config_path = "config.json";
        if let Ok(content) = fs::read_to_string(config_path) {
            if let Ok(config) = serde_json::from_str(&content) {
                return config;
            }
        }

        // Default Sane Config
        Self {
            margin_threshold_low: Decimal::new(4, 1),
            margin_threshold_high: Decimal::new(7, 1),
            concentration_threshold: Decimal::new(7, 1),
            target_margin_ratio: Decimal::new(2, 1),
            polling_interval_ms: 5000,
            automated_rebalance_enabled: false,
            wallets: HashMap::new(),
        }
    }

    pub fn _save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let content = serde_json::to_string_pretty(self)?;
        fs::write("config.json", content)?;
        Ok(())
    }
}
