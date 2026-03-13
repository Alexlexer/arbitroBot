mod constants;
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

use aggregator::Aggregator;
use execution::ExecutionActor;
use model::UnifiedTicker;
use rate_limiter::RateLimiter;
use tokio::sync::mpsc;
use std::sync::{Arc, Mutex};

#[tokio::main]
async fn main() {
    // Load Environment Variables
    dotenvy::dotenv().ok();

    // Setup Buffer Logger
    let log_capacity = 20;
    let logger = logger::BufferLogger::new(log_capacity);
    let log_buffer = logger.logs.clone(); // Clone Arc
    logger.init().expect("Failed to initialize logger");

    // Load Configuration
    let config = Arc::new(Mutex::new(config::AppConfig::load()));

    // Initialize Messaging (RabbitMQ)
    let messaging = messaging::init_messaging().await;

    // Setup Risk, Poller, Notifier & RateLimiter
    let risk_manager = risk_manager::RiskManager::new();
    let rebalance_advisor = rebalance_advisor::RebalanceAdvisor::new(&config.lock().unwrap_or_else(|e| e.into_inner()));
    let rate_limiter = Arc::new(RateLimiter::new());
    let poller = poller::DataPoller::new(rate_limiter.clone(), config.clone());
    let funding_rates = poller.funding_rates.clone();
    let market_filters = poller.market_filters.clone();
    let account_state = poller.account_state.clone();
    let notifier = Arc::new(notifier::TelegramNotifier::new(account_state.clone()));
    
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
    let (cmd_tx, cmd_rx) = mpsc::channel::<crate::model::BotCommand>(10);
    let (exec_tx, exec_rx) = mpsc::channel(100);

    // Setup Command Consumer
    let messaging_cmd = messaging.clone();
    tokio::spawn(async move {
        let m = messaging_cmd.lock().await;
        if let Some(client) = m.as_ref() {
            if let Err(e) = client.setup_command_consumer("bot_commands", "bot.commands", cmd_tx).await {
                log::error!("Failed to setup command consumer: {}", e);
            } else {
                log::info!("Command consumer started on queue 'bot_commands'");
            }
        }
    });

    // Run Exchange Launchers
    exchange::launch_all(tx).await;

    // Run Execution Actor (Background Thread)
    let mut execution_actor = ExecutionActor::new(exec_rx, rate_limiter.clone());
    tokio::spawn(async move {
        execution_actor.run().await;
    });

    // Run Aggregator (Main Thread)
    let account_state = poller_handle.account_state.clone();
    let mut aggregator = Aggregator::new(rx, cmd_rx, exec_tx, log_buffer, risk_manager, rebalance_advisor, funding_rates, market_filters, account_state, notifier, messaging, config);
    aggregator.run().await;
}
