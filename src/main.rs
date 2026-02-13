mod model;
mod exchange;
mod aggregator;
mod execution;
mod logger;
mod risk_manager;
mod poller;
mod notifier;
mod rate_limiter;

use aggregator::Aggregator;
use execution::ExecutionActor;
use model::UnifiedTicker;
use notifier::TelegramNotifier;
use rate_limiter::RateLimiter;
use tokio::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;

#[tokio::main]
async fn main() {
    // Load Environment Variables
    dotenvy::dotenv().ok();

    // Setup Buffer Logger
    let log_capacity = 20;
    let logger = logger::BufferLogger::new(log_capacity);
    let log_buffer = logger.logs.clone(); // Clone Arc
    logger.init().unwrap(); // Set as global logger

    // Setup Risk, Poller, Notifier & RateLimiter
    let risk_manager = risk_manager::RiskManager::new();
    let rate_limiter = Arc::new(RateLimiter::new());
    let poller = poller::DataPoller::new(rate_limiter.clone());
    let funding_rates = poller.funding_rates.clone();
    let market_filters = poller.market_filters.clone();
    let notifier = Arc::new(notifier::TelegramNotifier::new());
    
    // Spawn Funding Poller
    let poller_handle = Arc::new(poller);
    tokio::spawn(async move {
        poller_handle.run().await;
    });

    // Spawn Telegram Listener
    let notifier_listener = notifier.clone();
    tokio::spawn(async move {
        notifier_listener.run_listener().await;
    });

    // Channels
    let (tx, rx) = mpsc::channel::<UnifiedTicker>(1000);
    let (exec_tx, exec_rx) = mpsc::channel(100);

    // Run Exchange Launchers
    exchange::launch_all(tx).await;

    // Run Execution Actor (Background Thread)
    let mut execution_actor = ExecutionActor::new(exec_rx, rate_limiter.clone());
    tokio::spawn(async move {
        execution_actor.run().await;
    });

    // Run Aggregator (Main Thread)
    let mut aggregator = Aggregator::new(rx, exec_tx, log_buffer, risk_manager, funding_rates, market_filters, notifier);
    aggregator.run().await;
}
