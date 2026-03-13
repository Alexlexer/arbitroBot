use crate::model::{GlobalAccountState, ExchangeId, RebalanceAdvice};
use crate::config::AppConfig;
use rust_decimal::Decimal;

/// Min transfer amount (USDT) - below this, network fee (~$2) makes transfer uneconomical
fn min_transfer_after_fee_usdt() -> Decimal {
    Decimal::from(25)
}
/// Min surplus for concentration rebalance - higher threshold since it's advisory
fn min_concentration_transfer_usdt() -> Decimal {
    Decimal::from(100)
}

pub struct RebalanceAdvisor {
    margin_threshold_low: Decimal,
    margin_threshold_high: Decimal,
    concentration_threshold: Decimal,
    target_margin_ratio: Decimal,
}

impl RebalanceAdvisor {
    pub fn new(config: &AppConfig) -> Self {
        Self {
            margin_threshold_low: config.margin_threshold_low,
            margin_threshold_high: config.margin_threshold_high,
            concentration_threshold: config.concentration_threshold,
            target_margin_ratio: config.target_margin_ratio,
        }
    }

    pub fn check(&self, state: &GlobalAccountState) -> Vec<RebalanceAdvice> {
        let mut advices = Vec::new();

        if state.total_equity_usdt.is_zero() {
            return advices;
        }

        // 2. Collect all exchanges needing funds (Margin Pressure)
        let mut recipients = Vec::new();
        for (eid, ex_state) in &state.exchange_states {
            // Check if deposit is enabled for USDT
            let can_deposit = state.asset_statuses.get(eid)
                .and_then(|m| m.get("USDT"))
                .map(|s| s.can_deposit)
                .unwrap_or(true);

            if !can_deposit {
                continue;
            }

            if ex_state.margin_ratio > self.margin_threshold_low {
                let required = self.calculate_required_amount(ex_state.total_equity, ex_state.available_balance);
                if required > Decimal::ZERO {
                    recipients.push((*eid, ex_state.margin_ratio, required));
                }
            }
        }

        // Sort recipients by margin ratio descending (highest risk first)
        recipients.sort_by(|a, b| b.1.cmp(&a.1));

        // 3. Collect potential donors (Exchanges with surplus / low margin)
        let mut donors: Vec<_> = state.exchange_states.iter()
            .filter(|(eid, s)| {
                let can_withdraw = state.asset_statuses.get(eid)
                    .and_then(|m| m.get("USDT"))
                    .map(|s| s.can_withdraw)
                    .unwrap_or(true);
                
                can_withdraw && s.margin_ratio < self.margin_threshold_low && s.available_balance > Decimal::from(100)
            })
            .collect();
        
        // Sort donors by available balance descending (most surplus first)
        donors.sort_by(|a, b| b.1.available_balance.cmp(&a.1.available_balance));

        // 3. Match recipients with donors
        for (r_eid, r_ratio, mut r_amount) in recipients {
            for (d_eid, d_state) in &donors {
                if r_amount.is_zero() { break; }
                
                let donor_available = d_state.available_balance;
                if donor_available > Decimal::from(50) {
                    let transfer = r_amount.min(donor_available - Decimal::from(50)); // Leave some buffer
                    if transfer >= min_transfer_after_fee_usdt() {
                        let urgency = if r_ratio > self.margin_threshold_high { "URGENT" } else { "Advisory" };
                        advices.push(RebalanceAdvice {
                            from_exchange: **d_eid,
                            to_exchange: r_eid,
                            amount_usdt: transfer,
                            reason: format!("[{}] {} Margin Ratio is {:.1}% (Requires ${:.0})", urgency, r_eid, r_ratio * Decimal::from(100), r_amount),
                        });
                        r_amount -= transfer;
                    }
                }
            }
        }

        // 4. Check Concentration Risk (Special case)
        for (eid, ex_state) in &state.exchange_states {
            let concentration = ex_state.total_equity / state.total_equity_usdt;
            if concentration > self.concentration_threshold {
                let surplus = ex_state.total_equity - (state.total_equity_usdt * self.concentration_threshold);
                if surplus >= min_concentration_transfer_usdt() {
                    advices.push(RebalanceAdvice {
                        from_exchange: *eid,
                        to_exchange: ExchangeId::Binance, // Default safety net
                        amount_usdt: surplus,
                        reason: format!("Concentration Risk: {:.1}% on {} (Surplus ${:.0})", concentration * Decimal::from(100), eid, surplus),
                    });
                }
            }
        }

        advices
    }

    /// Calculates how much USDT is needed to bring margin ratio back to target (safe level)
    /// Margin Ratio = (Total Equity - Available Balance) / Total Equity
    /// We want (Total Equity + X - Available Balance) / (Total Equity + X) = Target
    /// Total Equity - Available Balance = Target * (Total Equity + X)
    /// Total Equity - Available Balance = Target * Total Equity + Target * X
    /// (1 - Target) * Total Equity - Available Balance = Target * X
    /// X = ((1 - Target) * Total Equity - Available Balance) / Target
    fn calculate_required_amount(&self, total_equity: Decimal, available: Decimal) -> Decimal {
        let factor = Decimal::ONE - self.target_margin_ratio;
        let required = (factor * total_equity - available) / self.target_margin_ratio;
        required.max(Decimal::ZERO).round_dp(0)
    }
}
