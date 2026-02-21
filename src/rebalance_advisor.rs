use crate::model::{GlobalAccountState, ExchangeId, RebalanceAdvice};
use rust_decimal::Decimal;

pub struct RebalanceAdvisor {
    margin_threshold_low: Decimal,   // 40% - Early warning
    margin_threshold_high: Decimal,  // 70% - Urgent
    concentration_threshold: Decimal, // 70% - Max capital on one exchange
}

impl RebalanceAdvisor {
    pub fn new() -> Self {
        Self {
            margin_threshold_low: Decimal::new(4, 1),   // 0.4
            margin_threshold_high: Decimal::new(7, 1),  // 0.7
            concentration_threshold: Decimal::new(7, 1), // 0.7
        }
    }

    pub fn check(&self, state: &GlobalAccountState) -> Vec<RebalanceAdvice> {
        let mut advices = Vec::new();

        if state.total_equity_usdt.is_zero() {
            return advices;
        }

        // 1. Collect all exchanges needing funds (Margin Pressure)
        let mut recipients = Vec::new();
        for (eid, ex_state) in &state.exchange_states {
            if ex_state.margin_ratio > self.margin_threshold_low {
                let required = self.calculate_required_amount(ex_state.total_equity, ex_state.available_balance);
                if required > Decimal::ZERO {
                    recipients.push((*eid, ex_state.margin_ratio, required));
                }
            }
        }

        // Sort recipients by margin ratio descending (highest risk first)
        recipients.sort_by(|a, b| b.1.cmp(&a.1));

        // 2. Collect potential donors (Exchanges with surplus / low margin)
        let mut donors: Vec<_> = state.exchange_states.iter()
            .filter(|(_, s)| s.margin_ratio < self.margin_threshold_low && s.available_balance > Decimal::from(100))
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
                    if transfer > Decimal::from(10) {
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
                if surplus > Decimal::from(100) {
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

    /// Calculates how much USDT is needed to bring margin ratio back to 20% (safe level)
    /// Margin Ratio = (Total Equity - Available Balance) / Total Equity
    /// We want (Total Equity + X - Available Balance) / (Total Equity + X) = 0.2
    /// Total Equity - Available Balance = 0.2 * (Total Equity + X)
    /// Total Equity - Available Balance = 0.2 * Total Equity + 0.2 * X
    /// 0.8 * Total Equity - Available Balance = 0.2 * X
    /// X = (0.8 * Total Equity - Available Balance) / 0.2
    fn calculate_required_amount(&self, total_equity: Decimal, available: Decimal) -> Decimal {
        let target_ratio = Decimal::new(2, 1); // target 20%
        let factor = Decimal::ONE - target_ratio; // 0.8
        let required = (factor * total_equity - available) / target_ratio;
        required.max(Decimal::ZERO).round_dp(0)
    }
}
