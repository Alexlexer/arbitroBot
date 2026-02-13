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
    Gate,
    Okx,
}

impl ExchangeId {
    pub fn taker_fee(&self) -> Decimal {
        match self {
            ExchangeId::Binance => Decimal::new(5, 4), // 0.0005 (0.05%)
            ExchangeId::Bybit => Decimal::new(6, 4),   // 0.0006 (0.06%)
            ExchangeId::Bitget => Decimal::new(6, 4),  // 0.0006 (0.06%)
            ExchangeId::Gate => Decimal::new(5, 4),    // 0.0005
            ExchangeId::Okx => Decimal::new(5, 4),     // 0.0005
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
}

#[derive(Debug, Clone)]
pub struct ArbitrageOpportunity {
    pub symbol: String,
    pub long_exchange: ExchangeId,
    pub short_exchange: ExchangeId,
    pub long_price: Decimal,
    pub short_price: Decimal,
    pub spread_pct: Decimal,
    pub _timestamp: i64,
}
