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

/// Per-exchange endpoint and tuning configuration.
/// All fields are optional — launchers fall back to built-in defaults when absent.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExchangeEndpointConfig {
    /// Primary WebSocket URL (e.g. "wss://fstream.binance.com/stream")
    pub ws_url: Option<String>,
    /// REST base URL (e.g. "https://fapi.binance.com")
    pub rest_url: Option<String>,
    /// Max number of symbols / subscriptions per connection
    pub max_symbols: Option<usize>,
    /// Poll interval in ms (for REST-polling launchers)
    pub poll_interval_ms: Option<u64>,
    /// Subscription batch size (for WS launchers that must batch subscribes)
    pub sub_batch_size: Option<usize>,
    /// Delay between subscription batches in ms
    pub sub_batch_delay_ms: Option<u64>,
    /// WebSocket keepalive ping interval in seconds
    pub ping_interval_secs: Option<u64>,
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
    #[serde(default)]
    pub live_trading_enabled: bool,
    pub wallets: HashMap<ExchangeId, String>,
    /// Per-exchange endpoint and tuning overrides.  Absent keys use launcher defaults.
    #[serde(default)]
    pub exchange_endpoints: HashMap<ExchangeId, ExchangeEndpointConfig>,
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
        enabled_exchanges.insert(ExchangeId::MEXC, false); // blocked: kept in registry but disabled
        enabled_exchanges.insert(ExchangeId::Bitmart, true);
        enabled_exchanges.insert(ExchangeId::Kraken, true);
        enabled_exchanges.insert(ExchangeId::Gate, true);
        enabled_exchanges.insert(ExchangeId::Okx, true);
        enabled_exchanges.insert(ExchangeId::Hyperliquid, true);
        enabled_exchanges.insert(ExchangeId::Aster, true);
        enabled_exchanges.insert(ExchangeId::Lighter, true);

        // Default endpoints — all fields explicit so the generated config.json is self-documenting
        let mut exchange_endpoints: HashMap<ExchangeId, ExchangeEndpointConfig> = HashMap::new();
        exchange_endpoints.insert(ExchangeId::Binance, ExchangeEndpointConfig {
            ws_url:             Some("wss://fstream.binance.com/stream".into()),
            rest_url:           Some("https://fapi.binance.com".into()),
            max_symbols:        Some(120),
            ..Default::default()
        });
        exchange_endpoints.insert(ExchangeId::Bybit, ExchangeEndpointConfig {
            ws_url:             Some("wss://stream.bybit.com/v5/public/linear".into()),
            rest_url:           Some("https://api.bybit.com".into()),
            max_symbols:        Some(120),
            sub_batch_size:     Some(50),
            sub_batch_delay_ms: Some(100),
            ..Default::default()
        });
        exchange_endpoints.insert(ExchangeId::Bitget, ExchangeEndpointConfig {
            ws_url:             Some("wss://ws.bitget.com/v2/ws/public".into()),
            rest_url:           Some("https://api.bitget.com".into()),
            max_symbols:        Some(120),
            sub_batch_size:     Some(40),
            sub_batch_delay_ms: Some(80),
            ping_interval_secs: Some(25),
            ..Default::default()
        });
        exchange_endpoints.insert(ExchangeId::MEXC, ExchangeEndpointConfig {
            ws_url:             Some("wss://contract.mexc.com/edge".into()),
            rest_url:           Some("https://contract.mexc.com".into()),
            max_symbols:        Some(120),
            sub_batch_size:     Some(20),
            ping_interval_secs: Some(20),
            ..Default::default()
        });
        exchange_endpoints.insert(ExchangeId::Bitmart, ExchangeEndpointConfig {
            rest_url:           Some("https://api-cloud-v2.bitmart.com/contract/public".into()),
            max_symbols:        Some(120),
            sub_batch_delay_ms: Some(200), // per-symbol depth request delay
            poll_interval_ms:   Some(2000),
            ..Default::default()
        });
        exchange_endpoints.insert(ExchangeId::Kraken, ExchangeEndpointConfig {
            rest_url:           Some("https://futures.kraken.com/derivatives/api/v3".into()),
            max_symbols:        Some(120),
            poll_interval_ms:   Some(2000),
            ..Default::default()
        });
        exchange_endpoints.insert(ExchangeId::Gate, ExchangeEndpointConfig {
            ws_url:             Some("wss://fx-ws.gateio.ws/v4/ws/usdt".into()),
            rest_url:           Some("https://fx-api.gateio.ws/api/v4".into()),
            max_symbols:        Some(120),
            sub_batch_size:     Some(20),
            sub_batch_delay_ms: Some(100),
            ping_interval_secs: Some(15),
            ..Default::default()
        });
        exchange_endpoints.insert(ExchangeId::Hyperliquid, ExchangeEndpointConfig {
            ws_url:             Some("wss://api.hyperliquid.xyz/ws".into()),
            rest_url:           Some("https://api.hyperliquid.xyz".into()),
            max_symbols:        Some(100),
            ping_interval_secs: Some(30),
            ..Default::default()
        });
        exchange_endpoints.insert(ExchangeId::Lighter, ExchangeEndpointConfig {
            rest_url:           Some("https://mainnet.zklighter.elliot.ai".into()),
            poll_interval_ms:   Some(500),
            ..Default::default()
        });
        exchange_endpoints.insert(ExchangeId::Aster, ExchangeEndpointConfig {
            rest_url:           Some("https://api.aster.finance".into()),
            poll_interval_ms:   Some(500),
            ..Default::default()
        });

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
            live_trading_enabled: false,
            wallets: HashMap::new(),
            exchange_endpoints,
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
    pub bitget_passphrase: Option<String>,
    pub gate_key: Option<String>,
    pub gate_secret: Option<String>,
    pub bitmart_key: Option<String>,
    pub bitmart_secret: Option<String>,
    pub bitmart_memo: Option<String>,
    pub kraken_key: Option<String>,
    pub kraken_secret: Option<String>,
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
