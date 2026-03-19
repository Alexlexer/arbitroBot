use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum RiskError {
    LowLiquidity(String),
    HighFundingLoss(String),
    WalletDisabled(String),
    _PrecisionMismatch(String),
    PriceDrift(String),
    MinNotionalFilter(String),
    LiquidationRisk(String),
    ExchangeError(String),
}

impl fmt::Display for RiskError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RiskError::LowLiquidity(msg) => write!(f, "Liquidity Risk: {}", msg),
            RiskError::HighFundingLoss(msg) => write!(f, "Funding Risk: {}", msg),
            RiskError::WalletDisabled(msg) => write!(f, "Wallet Risk: {}", msg),
            RiskError::_PrecisionMismatch(msg) => write!(f, "Precision Risk: {}", msg),
            RiskError::PriceDrift(msg) => write!(f, "Price Drift: {}", msg),
            RiskError::MinNotionalFilter(msg) => write!(f, "Min Notional Risk: {}", msg),
            RiskError::LiquidationRisk(msg) => write!(f, "Liquidation Risk: {}", msg),
            RiskError::ExchangeError(msg) => write!(f, "Exchange Error: {}", msg),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookDepth {
    pub bids: Vec<(Decimal, Decimal)>, // Price, Size
    pub asks: Vec<(Decimal, Decimal)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundingInfo {
    pub rate_pct: Decimal, // e.g. 0.01 for 0.01%
    pub next_funding_time: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetStatus {
    pub can_deposit: bool,
    pub can_withdraw: bool,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolMarketFilters {
    pub min_notional: Decimal, // Minimum USDT value (or quote currency)
    pub is_trading: bool,      // Whether the symbol is currently in active trading mode
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExchangeId {
    Binance,
    Bybit,
    Bitget,
    MEXC,
    Bitmart,
    Kraken,
    Gate,
    Okx,
}

impl ExchangeId {
    pub fn taker_fee(&self) -> Decimal {
        match self {
            ExchangeId::Binance => Decimal::new(5, 4), // 0.0005 (0.05%)
            ExchangeId::Bybit => Decimal::new(6, 4),   // 0.0006 (0.06%)
            ExchangeId::Bitget => Decimal::new(6, 4), // 0.06%
            ExchangeId::MEXC => Decimal::new(1, 3),   // 0.1%
            ExchangeId::Bitmart => Decimal::new(1, 3), // 0.1%
            ExchangeId::Kraken => Decimal::new(2, 3),  // 0.2%
            ExchangeId::Gate => Decimal::new(5, 4),    // 0.05%
            ExchangeId::Okx => Decimal::new(5, 4),     // 0.05%
        }
    }
}

impl fmt::Display for ExchangeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedTicker {
    pub symbol: String,
    pub exchange: ExchangeId,
    pub timestamp: i64,
    pub bids: Vec<(Decimal, Decimal)>, // Price, Size
    pub asks: Vec<(Decimal, Decimal)>, // Price, Size
}

impl UnifiedTicker {
    pub fn best_bid(&self) -> Option<(Decimal, Decimal)> {
        self.bids.first().cloned()
    }
    pub fn best_ask(&self) -> Option<(Decimal, Decimal)> {
        self.asks.first().cloned()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionInfo {
    pub symbol: String,
    pub side: String, // "LONG" or "SHORT"
    pub size: Decimal,
    pub entry_price: Decimal,
    pub unrealized_pnl: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeAccountState {
    pub total_equity: Decimal,
    pub available_balance: Decimal,
    pub margin_ratio: Decimal, // e.g. 0.05 for 5%
    pub positions: Vec<PositionInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalAccountState {
    pub total_equity_usdt: Decimal,
    pub total_unrealized_pnl: Decimal,
    pub exchange_states: HashMap<ExchangeId, ExchangeAccountState>,
    pub asset_statuses: HashMap<ExchangeId, HashMap<String, AssetStatus>>,
}

#[derive(Debug, Clone)]
pub struct ArbitrageOpportunity {
    pub symbol: String,
    pub long_exchange: ExchangeId,
    pub short_exchange: ExchangeId,
    pub long_price: Decimal,
    pub short_price: Decimal,
    pub spread_pct: Decimal,
    pub volume_usdt: Decimal,
    pub _timestamp: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RebalanceAdvice {
    pub from_exchange: ExchangeId,
    pub to_exchange: ExchangeId,
    pub amount_usdt: Decimal, // Recommended transfer
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiCredentials {
    pub key: String,
    pub secret: String,
    pub passphrase: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BotCommand {
    UpdateSpread { threshold: Decimal },
    /// Update target depth in quote currency (USDT) for VWAP/liquidity evaluation.
    UpdateDepth { depth_usdt: Decimal },
    /// Update Listener WS URL (e.g. ws://host:8083/ws). Empty string disables listener.
    UpdateListenerWsUrl { url: String },
    ToggleExchange { exchange: ExchangeId, enabled: bool },
    UpdateApiKeys { exchange: ExchangeId, credentials: ApiCredentials },
    /// Dashboard login: bot validates username + password, publishes to dashboard.auth
    DashboardLogin { username: String, password: String, request_id: String },
    /// Dashboard register: requires invite_code, then creates user
    DashboardRegister { username: String, password: String, invite_code: String, request_id: String },
}

/// One row for dashboard history API (serializable).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryOpportunity {
    pub symbol: String,
    pub long_exchange: String,
    pub long_price: f64,
    pub short_exchange: String,
    pub short_price: f64,
    pub spread: f64,
}

/// Incoming message from Listener WS (`type: "alert"`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListenerAlert {
    pub symbol: String,
    pub exchange: String,
    pub kind: String,
    pub source: String,
    pub reason: String,
    pub at: i64,
    #[serde(default)]
    pub mcap_usd: Option<f64>,
}

/// One computed row to show on the dashboard for a Listener-triggered symbol.
/// Field names are camelCase because the dashboard matrix row expects:
/// `longExchange`, `longPrice`, `shortExchange`, `shortPrice`, `spread`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListenerOpportunityPayload {
    pub symbol: String,
    #[serde(rename = "longExchange")]
    pub long_exchange: String,
    #[serde(rename = "longPrice")]
    pub long_price: f64,
    #[serde(rename = "shortExchange")]
    pub short_exchange: String,
    #[serde(rename = "shortPrice")]
    pub short_price: f64,
    pub spread: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardAuthResponse {
    pub ok: bool,
    pub request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub fn normalize_symbol(s: &str) -> String {
    s.to_uppercase()
        .replace("XBT", "BTC")
        .replace("_USDT", "")
        .replace("USDT", "")
        .replace("USD", "")
        .replace("_", "")
        .replace("-", "")
        .replace("/", "")
}
