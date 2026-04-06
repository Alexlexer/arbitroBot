use async_trait::async_trait;
use tokio::sync::mpsc::Sender;
use crate::model::{ExchangeId, UnifiedTicker};
use crate::config::{AppConfig, ExchangeEndpointConfig};

pub mod binance;
pub mod bybit;
pub mod gate;
pub mod hyperliquid;
pub mod aster;

/// Exponential backoff delay for WebSocket reconnection (secs). Caps at 120s.
pub fn reconnect_delay_secs(attempt: u32) -> u64 {
    (2u64.pow(attempt.min(6))).min(120)
}

#[async_trait]
pub trait Exchange: Send + Sync {
    async fn connect(&mut self, tx: Sender<UnifiedTicker>) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    #[allow(dead_code)]
    async fn subscribe(&mut self, symbols: &[String]) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

/// Snapshot endpoint config for a given exchange, falling back to an empty default.
fn ep(config: &AppConfig, id: ExchangeId) -> ExchangeEndpointConfig {
    config.exchange_endpoints.get(&id).cloned().unwrap_or_default()
}

pub async fn launch_all(tx: Sender<UnifiedTicker>, config: &AppConfig) {
    use crate::exchange::binance::BinanceLauncher;
    use crate::exchange::bybit::BybitLauncher;
    use crate::exchange::hyperliquid::HyperliquidLauncher;
    use crate::exchange::aster::AsterLauncher;

    // Returns true if the exchange is enabled (absent key defaults to enabled).
    let enabled = |id: ExchangeId| -> bool {
        config.enabled_exchanges.get(&id).copied().unwrap_or(true)
    };

    macro_rules! spawn_launcher {
        ($id:expr, $launcher:expr, $tx:expr, $name:literal) => {{
            if enabled($id) {
                let tx_clone = $tx.clone();
                let mut launcher = $launcher;
                tokio::spawn(async move {
                    if let Err(e) = launcher.connect(tx_clone).await {
                        log::error!("{} launch failed: {}", $name, e);
                    }
                });
            } else {
                log::info!("{} is disabled in config — skipping launch.", $name);
            }
        }};
    }

    spawn_launcher!(ExchangeId::Binance,     BinanceLauncher::new(&ep(config, ExchangeId::Binance)),     tx, "Binance");
    spawn_launcher!(ExchangeId::Bybit,       BybitLauncher::new(&ep(config, ExchangeId::Bybit)),         tx, "Bybit");

    if enabled(ExchangeId::Gate) {
        let tx_gate = tx.clone();
        let mut gate = crate::exchange::gate::GateLauncher::new(&ep(config, ExchangeId::Gate));
        tokio::spawn(async move {
            if let Err(e) = gate.connect(tx_gate).await {
                log::error!("Gate launch failed: {}", e);
            }
        });
    } else {
        log::info!("Gate is disabled in config — skipping launch.");
    }

    spawn_launcher!(ExchangeId::Hyperliquid, HyperliquidLauncher::new(&ep(config, ExchangeId::Hyperliquid)), tx, "Hyperliquid");
    spawn_launcher!(ExchangeId::Aster,       AsterLauncher::new(&ep(config, ExchangeId::Aster)),             tx, "Aster");
}
