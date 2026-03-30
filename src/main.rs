mod model;
mod exchange;
mod aggregator;
mod execution;
mod logger;
mod risk_manager;
mod poller;
mod notifier;
mod rate_limiter;
mod rebalance_advisor;
mod config;
mod messaging;
mod transfers;

use aggregator::Aggregator;
use execution::ExecutionActor;
use model::UnifiedTicker;
use rate_limiter::RateLimiter;
use tokio::sync::mpsc;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    // Load Environment Variables
    dotenvy::dotenv().ok();

    // Load Configuration
    let config = config::AppConfig::load();
    let secrets = config::SecretsConfig::load();

    // Initialize Messaging (RabbitMQ)
    let messaging = messaging::init_messaging().await;

    // Setup Buffer Logger
    let log_capacity = 20;
    let logger = logger::BufferLogger::new(log_capacity);
    let log_buffer = logger.logs.clone(); // Clone Arc
    logger.init().unwrap(); // Set as global logger

    // Setup Risk, Poller, Notifier & RateLimiter
    let config_arc = Arc::new(std::sync::Mutex::new(config.clone()));
    let secrets_arc = Arc::new(std::sync::Mutex::new(secrets.clone()));
    
    let risk_manager = risk_manager::RiskManager::new(config_arc.clone(), secrets_arc.clone());
    let rebalance_advisor = rebalance_advisor::RebalanceAdvisor::new(&config);
    let rate_limiter = Arc::new(RateLimiter::new());
    let client = reqwest::Client::new();
    let poller = poller::DataPoller::new(rate_limiter.clone(), config_arc.clone(), secrets_arc.clone(), client);
    let funding_rates = poller.funding_rates.clone();
    let market_filters = poller.market_filters.clone();
    let account_state = poller.account_state.clone();
    let notifier = Arc::new(notifier::TelegramNotifier::new(account_state.clone(), config_arc.clone(), secrets_arc.clone()));
    
    // Spawn Data Pollers
    let poller_handle = Arc::new(poller);
    let p_pub = poller_handle.clone();
    tokio::spawn(async move {
        p_pub.run().await;
    });
    let p_priv = poller_handle.clone();
    tokio::spawn(async move {
        p_priv.run_private().await;
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
    let account_state = poller_handle.account_state.clone();
    let mut aggregator = Aggregator::new(rx, exec_tx, log_buffer, risk_manager, rebalance_advisor, funding_rates, market_filters, account_state, notifier, messaging.clone());
    
    // Start Command Consumer
    {
        let msg = messaging.lock().await;
        if let Some(client) = msg.as_ref() {
            let consumer = aggregator.get_command_consumer();
            let _ = client.consume("arbit_hub_commands", "commands", consumer).await;
        }
    }

    aggregator.run().await;
}
