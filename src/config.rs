use serde::{Deserialize, Serialize};
use std::fs;
use rust_decimal::Decimal;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub margin_threshold_low: Decimal,    // 0.4 (40%)
    pub margin_threshold_high: Decimal,   // 0.7 (70%)
    pub concentration_threshold: Decimal, // 0.7 (70%)
    pub target_margin_ratio: Decimal,     // 0.2 (20%)
    pub polling_interval_ms: u64,         // 5000
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
        }
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let content = serde_json::to_string_pretty(self)?;
        fs::write("config.json", content)?;
        Ok(())
    }
}
