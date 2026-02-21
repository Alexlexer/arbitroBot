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

        // 1. Check Concentration Risk
        for (eid, ex_state) in &state.exchange_states {
            let concentration = ex_state.total_equity / state.total_equity_usdt;
            if concentration > self.concentration_threshold {
                advices.push(RebalanceAdvice {
                    from_exchange: *eid,
                    to_exchange: ExchangeId::Binance, // Default suggestions for now
                    amount_usdt: ex_state.total_equity * Decimal::new(2, 1), // Suggest moving 20%
                    reason: format!("High Concentration Risk: {:.1}% on {}", concentration * Decimal::from(100), eid),
                });
            }
        }

        // 2. Check Margin Pressure
        for (eid, ex_state) in &state.exchange_states {
            if ex_state.margin_ratio > self.margin_threshold_low {
                let urgency = if ex_state.margin_ratio > self.margin_threshold_high { "URGENT" } else { "Advisory" };
                
                // Find a donor (exchange with lowest margin ratio and available balance)
                let mut donor = None;
                let mut min_ratio = Decimal::ONE;

                for (deid, dex_state) in &state.exchange_states {
                    if deid != eid && dex_state.margin_ratio < min_ratio && dex_state.available_balance > Decimal::from(100) {
                        min_ratio = dex_state.margin_ratio;
                        donor = Some(*deid);
                    }
                }

                if let Some(from_eid) = donor {
                    advices.push(RebalanceAdvice {
                        from_exchange: from_eid,
                        to_exchange: *eid,
                        amount_usdt: Decimal::from(500), // Default rebalance chunk
                        reason: format!("[{}] {} Margin Ratio is {:.1}%", urgency, eid, ex_state.margin_ratio * Decimal::from(100)),
                    });
                }
            }
        }

        advices
    }
}
