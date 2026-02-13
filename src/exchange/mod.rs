use async_trait::async_trait;
use tokio::sync::mpsc::Sender;
use crate::model::UnifiedTicker;

pub mod binance;
pub mod bybit;
pub mod bitget;

#[async_trait]
pub trait Exchange: Send + Sync {
    async fn connect(&mut self, tx: Sender<UnifiedTicker>) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    async fn subscribe(&mut self, symbols: &[String]) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

pub async fn launch_all(tx: Sender<UnifiedTicker>) {
    use crate::exchange::binance::BinanceLauncher;
    use crate::exchange::bybit::BybitLauncher;
    use crate::exchange::bitget::BitgetLauncher;
    use crate::exchange::Exchange;

    // Binance
    let tx_binance = tx.clone();
    tokio::spawn(async move {
        let mut binance = BinanceLauncher;
        if let Err(e) = binance.connect(tx_binance).await {
            log::error!("Binance launch failed: {}", e);
        }
    });

    // Bybit
    let tx_bybit = tx.clone();
    tokio::spawn(async move {
        let mut bybit = BybitLauncher;
        if let Err(e) = bybit.connect(tx_bybit).await {
            log::error!("Bybit launch failed: {}", e);
        }
    });

    // Bitget
    let tx_bitget = tx.clone();
    tokio::spawn(async move {
        let mut bitget = BitgetLauncher;
        if let Err(e) = bitget.connect(tx_bitget).await {
            log::error!("Bitget launch failed: {}", e);
        }
    });
}
