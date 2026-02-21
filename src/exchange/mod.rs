use async_trait::async_trait;
use tokio::sync::mpsc::Sender;
use crate::model::UnifiedTicker;

pub mod binance;
pub mod bybit;
pub mod bitget;
pub mod mexc;
pub mod bitmart;
pub mod kraken;
pub mod ourbit;
pub mod gate;

#[async_trait]
pub trait Exchange: Send + Sync {
    async fn connect(&mut self, tx: Sender<UnifiedTicker>) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    #[allow(dead_code)]
    async fn subscribe(&mut self, symbols: &[String]) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

pub async fn launch_all(tx: Sender<UnifiedTicker>) {
    use crate::exchange::binance::BinanceLauncher;
    use crate::exchange::bybit::BybitLauncher;
    use crate::exchange::bitget::BitgetLauncher;
    use crate::exchange::mexc::MexcLauncher;
    use crate::exchange::bitmart::BitmartLauncher;
    use crate::exchange::kraken::KrakenLauncher;
    use crate::exchange::ourbit::OurbitLauncher;
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

    // MEXC
    let tx_mexc = tx.clone();
    tokio::spawn(async move {
        let mut mexc = MexcLauncher;
        if let Err(e) = mexc.connect(tx_mexc).await {
            log::error!("MEXC launch failed: {}", e);
        }
    });

    // Bitmart
    let tx_bitmart = tx.clone();
    tokio::spawn(async move {
        let mut bitmart = BitmartLauncher;
        if let Err(e) = bitmart.connect(tx_bitmart).await {
            log::error!("Bitmart launch failed: {}", e);
        }
    });

    // Kraken
    let tx_kraken = tx.clone();
    tokio::spawn(async move {
        let mut kraken = KrakenLauncher;
        if let Err(e) = kraken.connect(tx_kraken).await {
            log::error!("Kraken launch failed: {}", e);
        }
    });

    // Ourbit
    let tx_ourbit = tx.clone();
    tokio::spawn(async move {
        let mut ourbit = OurbitLauncher;
        if let Err(e) = ourbit.connect(tx_ourbit).await {
            log::error!("Ourbit launch failed: {}", e);
        }
    });

    // Gate.io
    let tx_gate = tx.clone();
    tokio::spawn(async move {
        let mut gate = crate::exchange::gate::GateLauncher;
        if let Err(e) = gate.connect(tx_gate).await {
            log::error!("Gate launch failed: {}", e);
        }
    });
}
