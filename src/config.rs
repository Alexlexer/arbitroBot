use serde::{Deserialize, Serialize};
use std::fs;
use rust_decimal::Decimal;

fn default_use_vwap_pricing() -> bool {
    true
}

fn default_max_position_pct() -> Decimal {
    Decimal::new(5, 2) // 0.05 = 5% of total equity per trade
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub margin_threshold_low: Decimal,    // 0.4 (40%)
    pub margin_threshold_high: Decimal,   // 0.7 (70%)
    pub concentration_threshold: Decimal, // 0.7 (70%)
    pub target_margin_ratio: Decimal,     // 0.2 (20%)
    pub polling_interval_ms: u64,         // 5000
    pub min_spread_threshold: Decimal,    // 5.0 (5%)
    /// Target depth in quote currency (USDT) to use when evaluating order book liquidity.
    /// Currently only propagated to dashboard; pricing logic still uses best bid/ask.
    #[serde(default)]
    pub depth_usdt: Decimal,
    /// Use VWAP-on-depth for spread; if false, best bid/ask is used (fallback for debugging).
    #[serde(default = "default_use_vwap_pricing")]
    pub use_vwap_pricing: bool,
    /// RabbitMQ is for dashboard <-> bot. This WS is for incoming listing alerts (Listener service).
    /// Empty / None disables Listener integration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listener_ws_url: Option<String>,
    /// Per-exchange taker fee overrides (decimal, e.g. 0.0004 = 0.04%).
    /// If absent for an exchange, the compiled-in default is used.
    #[serde(default)]
    pub taker_fee_overrides: std::collections::HashMap<crate::model::ExchangeId, Decimal>,
    /// Max fraction of total equity risked per single trade (e.g. 0.05 = 5%).
    #[serde(default = "default_max_position_pct")]
    pub max_position_pct: Decimal,
    pub enabled_exchanges: std::collections::HashMap<crate::model::ExchangeId, bool>,
    /// Never serialized to or deserialized from config.json (secrets stay in .env / memory only)
    #[serde(skip_serializing, skip_deserializing, default)]
    pub api_keys: std::collections::HashMap<crate::model::ExchangeId, crate::model::ApiCredentials>,
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
        let mut enabled_exchanges = std::collections::HashMap::new();
        use crate::model::ExchangeId as E;
        enabled_exchanges.insert(E::Binance, true);
        enabled_exchanges.insert(E::Bybit, true);
        enabled_exchanges.insert(E::Bitget, true);
        enabled_exchanges.insert(E::MEXC, true);
        enabled_exchanges.insert(E::Bitmart, true);
        enabled_exchanges.insert(E::Kraken, true);
        enabled_exchanges.insert(E::Gate, true);
        enabled_exchanges.insert(E::Okx, true);

        Self {
            margin_threshold_low: Decimal::new(4, 1),
            margin_threshold_high: Decimal::new(7, 1),
            concentration_threshold: Decimal::new(7, 1),
            target_margin_ratio: Decimal::new(2, 1),
            polling_interval_ms: 5000,
            min_spread_threshold: Decimal::from(5),
             // Sensible starting depth; can be changed from dashboard.
            depth_usdt: Decimal::from(50),
            use_vwap_pricing: true,
            listener_ws_url: None,
            taker_fee_overrides: std::collections::HashMap::new(),
            max_position_pct: default_max_position_pct(),
            enabled_exchanges,
            api_keys: std::collections::HashMap::new(),
        }
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let content = serde_json::to_string_pretty(self)?;
        fs::write("config.json", content)?;
        Ok(())
    }
}
