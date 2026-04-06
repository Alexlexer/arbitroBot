use std::collections::HashMap;
use tokio::sync::Mutex;
use tokio::time::{Instant};
use crate::model::ExchangeId;
use log::warn;

pub struct TokenBucket {
    tokens: f64,
    max_tokens: f64,
    refill_rate: f64, // tokens per second
    last_refill: Instant,
}

impl TokenBucket {
    pub fn new(max_tokens: f64, refill_rate: f64) -> Self {
        Self {
            tokens: max_tokens,
            max_tokens,
            refill_rate,
            last_refill: Instant::now(),
        }
    }

    pub fn acquire(&mut self, weight: f64) -> bool {
        self.refill();
        if self.tokens >= weight {
            self.tokens -= weight;
            true
        } else {
            false
        }
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        let add_tokens = elapsed * self.refill_rate;
        self.tokens = (self.tokens + add_tokens).min(self.max_tokens);
        self.last_refill = now;
    }

    pub fn _available_tokens(&self) -> f64 {
        self.tokens
    }
}

pub struct ExchangeRateLimiter {
    rest_bucket: Mutex<TokenBucket>,
    order_bucket: Mutex<TokenBucket>,
}

impl ExchangeRateLimiter {
    pub fn new(rest_max: f64, rest_refill: f64, order_max: f64, order_refill: f64) -> Self {
        Self {
            rest_bucket: Mutex::new(TokenBucket::new(rest_max, rest_refill)),
            order_bucket: Mutex::new(TokenBucket::new(order_max, order_refill)),
        }
    }

    pub async fn acquire_rest(&self, weight: f64) -> bool {
        self.rest_bucket.lock().await.acquire(weight)
    }

    pub async fn acquire_order(&self, weight: f64) -> bool {
        self.order_bucket.lock().await.acquire(weight)
    }
}

pub struct RateLimiter {
    limiters: HashMap<ExchangeId, ExchangeRateLimiter>,
}

impl RateLimiter {
    pub fn new() -> Self {
        let mut limiters = HashMap::new();

        // Binance: 1200 weight per minute -> 20/sec
        // Orders: 10 per sec
        limiters.insert(ExchangeId::Binance, ExchangeRateLimiter::new(1200.0, 20.0, 10.0, 10.0));

        // Bybit: 50 orders per second (usually)
        // General: 10/sec
        limiters.insert(ExchangeId::Bybit, ExchangeRateLimiter::new(100.0, 10.0, 50.0, 50.0));

        Self { limiters }
    }

    pub async fn check_limit(&self, exchange: ExchangeId, is_order: bool, weight: f64) -> bool {
        if let Some(limiter) = self.limiters.get(&exchange) {
            if is_order {
                if !limiter.acquire_order(weight).await {
                    warn!("Rate limit exceeded for {} (ORDER)", exchange);
                    return false;
                }
            } else {
                if !limiter.acquire_rest(weight).await {
                    warn!("Rate limit exceeded for {} (REST)", exchange);
                    return false;
                }
            }
            true
        } else {
            true // No limiter for unknown exchange
        }
    }
}
