use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

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
    Gate,
    Hyperliquid,
    Aster,
}

impl ExchangeId {
    pub fn taker_fee(&self) -> Decimal {
        match self {
            ExchangeId::Binance => Decimal::new(5, 4),      // 0.05%
            ExchangeId::Bybit => Decimal::new(6, 4),        // 0.06%
            ExchangeId::Gate => Decimal::new(5, 4),         // 0.05%
            ExchangeId::Hyperliquid => Decimal::new(35, 5), // 0.035%
            ExchangeId::Aster => Decimal::new(5, 4),        // 0.05%
        }
    }

    pub fn effective_taker_fee(&self, overrides: &HashMap<ExchangeId, Decimal>) -> Decimal {
        overrides
            .get(self)
            .copied()
            .unwrap_or_else(|| self.taker_fee())
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
    pub config: crate::config::AppConfig,
    pub secrets: crate::config::SecretsConfig,
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
pub struct HistoryOpportunity {
    pub symbol: String,
    pub long_exchange: String,
    pub long_price: f64,
    pub short_exchange: String,
    pub short_price: f64,
    pub spread: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListenerAlert {
    pub kind: String,
    pub symbol: String,
    pub exchange: String,
    pub source: String,
    pub reason: String,
    pub at: i64,
    pub mcap_usd: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListenerOpportunityPayload {
    pub symbol: String,
    pub long_exchange: String,
    pub long_price: f64,
    pub short_exchange: String,
    pub short_price: f64,
    pub spread: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardAuthResponse {
    pub ok: bool,
    pub request_id: String,
    pub username: Option<String>,
    pub error: Option<String>,
}

/// Commands sent from the dashboard via RabbitMQ.
/// Serialised with internally-tagged format so the JSON key "type" drives dispatch,
/// e.g. {"type":"update_spread","threshold":"0.5"}.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BotCommand {
    UpdateSpread {
        threshold: Decimal,
    },
    UpdateDepth {
        depth_usdt: Decimal,
    },
    UpdateListenerWsUrl {
        url: String,
    },
    ToggleExchange {
        exchange: ExchangeId,
        enabled: bool,
    },
    UpdateApiKeys {
        exchange: ExchangeId,
        credentials: crate::config::ExchangeCredentials,
    },
    UpdateFees {
        exchange: ExchangeId,
        fee: Decimal,
    },
    UpdateBlacklist {
        symbols: Vec<String>,
    },
    DashboardLogin {
        username: String,
        password: String,
        request_id: String,
    },
    DashboardRegister {
        username: String,
        password: String,
        invite_code: String,
        request_id: String,
    },
    UpdateConfig {
        config: crate::config::AppConfig,
    },
    UpdateSecrets {
        secrets: crate::config::SecretsConfig,
    },
    EmergencyStop,
    Resume,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TradeStatus {
    Filled,
    Failed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeRecord {
    pub id: String,
    pub symbol: String,
    pub long_exchange: ExchangeId,
    pub short_exchange: ExchangeId,
    pub long_price: Decimal,
    pub short_price: Decimal,
    pub volume_usdt: Decimal,
    pub spread_pct: Decimal,
    pub long_status: TradeStatus,
    pub short_status: TradeStatus,
    pub realized_pnl_usdt: Option<Decimal>,
    pub timestamp: i64,
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
