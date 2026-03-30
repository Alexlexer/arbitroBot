use crate::model::ExchangeId;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeCredentials {
    pub key: String,
    pub secret: String,
    pub passphrase: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub min_spread_threshold: Decimal,
    pub depth_usdt: Decimal,
    pub listener_ws_url: Option<String>,
    pub enabled_exchanges: HashMap<ExchangeId, bool>,
    pub api_keys: HashMap<ExchangeId, ExchangeCredentials>,
    pub taker_fee_overrides: HashMap<ExchangeId, Decimal>,
    pub margin_threshold_low: Decimal,
    pub margin_threshold_high: Decimal,
    pub concentration_threshold: Decimal,
    pub target_margin_ratio: Decimal,
    pub polling_interval_ms: u64,
    pub automated_rebalance_enabled: bool,
    pub wallets: HashMap<ExchangeId, String>,
}

impl AppConfig {
    pub fn load() -> Self {
        let config_path = "config.json";
        if let Ok(content) = fs::read_to_string(config_path) {
            if let Ok(config) = serde_json::from_str(&content) {
                return config;
            }
        }

        // Default Config
        let mut enabled_exchanges = HashMap::new();
        enabled_exchanges.insert(ExchangeId::Binance, true);
        enabled_exchanges.insert(ExchangeId::Bybit, true);
        enabled_exchanges.insert(ExchangeId::Bitget, true);
        enabled_exchanges.insert(ExchangeId::MEXC, true);
        enabled_exchanges.insert(ExchangeId::Bitmart, true);
        enabled_exchanges.insert(ExchangeId::Kraken, true);
        enabled_exchanges.insert(ExchangeId::Gate, true);
        enabled_exchanges.insert(ExchangeId::Okx, true);

        Self {
            min_spread_threshold: Decimal::new(5, 0),
            depth_usdt: Decimal::new(1000, 0),
            listener_ws_url: None,
            enabled_exchanges,
            api_keys: HashMap::new(),
            taker_fee_overrides: HashMap::new(),
            margin_threshold_low: Decimal::new(4, 1),
            margin_threshold_high: Decimal::new(7, 1),
            concentration_threshold: Decimal::new(7, 1),
            target_margin_ratio: Decimal::new(2, 1),
            polling_interval_ms: 5000,
            automated_rebalance_enabled: false,
            wallets: HashMap::new(),
        }
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let content = serde_json::to_string_pretty(self)?;
        fs::write("config.json", content)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SecretsConfig {
    pub binance_key: Option<String>,
    pub binance_secret: Option<String>,
    pub bybit_key: Option<String>,
    pub bybit_secret: Option<String>,
    pub bitget_key: Option<String>,
    pub bitget_secret: Option<String>,
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
        let secrets_path = "secrets.json";
        if let Ok(content) = fs::read_to_string(secrets_path) {
            if let Ok(secrets) = serde_json::from_str(&content) {
                return secrets;
            }
        }
        Self::default()
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let content = serde_json::to_string_pretty(self)?;
        fs::write("secrets.json", content)?;
        Ok(())
    }
}
