mod auth;
mod constants;
mod model;
mod exchange;
mod aggregator;
mod execution;
mod history;
mod logger;
mod risk_manager;
mod poller;
mod notifier;
mod rate_limiter;
mod rebalance_advisor;
mod config;
mod messaging;
mod transfers;
mod listener;

use aggregator::Aggregator;
use execution::ExecutionActor;
use model::{UnifiedTicker, HistoryOpportunity, ListenerAlert};
use rate_limiter::RateLimiter;
use tokio::sync::{mpsc, watch};
use std::sync::{Arc, Mutex};
use std::path::Path;

#[tokio::main]
async fn main() {
    // Load Environment Variables
    dotenvy::dotenv().ok();

    // Load Configuration
    let initial_config = config::AppConfig::load();
    let secrets = config::SecretsConfig::load();
    let config = Arc::new(Mutex::new(initial_config));
    let secrets = Arc::new(Mutex::new(secrets));

    // Initialize Messaging (RabbitMQ)
    let messaging = messaging::init_messaging().await;

    // Setup Buffer Logger
    let log_capacity = 20;
    let logger = logger::BufferLogger::new(log_capacity);
    let log_buffer = logger.logs.clone(); // Clone Arc
    logger.init().expect("Failed to initialize logger");

    // Listener integration (optional; can be configured from dashboard via BotCommand)
    let initial_listener_url = config
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .listener_ws_url
        .clone()
        .unwrap_or_default();
    let (listener_url_tx, listener_url_rx) = watch::channel::<String>(initial_listener_url);
    let (listener_alert_tx, listener_alert_rx) = mpsc::channel::<ListenerAlert>(64);

    tokio::spawn(async move {
        listener::run_listener_client(listener_url_rx, listener_alert_tx).await;
    });

    // Setup Risk, Poller, Notifier & RateLimiter
    let risk_manager = risk_manager::RiskManager::new(config.clone(), secrets.clone());
    let rebalance_advisor = rebalance_advisor::RebalanceAdvisor::new(&config.lock().unwrap_or_else(|e| e.into_inner()));
    let rate_limiter = Arc::new(RateLimiter::new());
    let client = reqwest::Client::new();
    
    let poller = poller::DataPoller::new(rate_limiter.clone(), config.clone(), secrets.clone(), client);
    let funding_rates = poller.funding_rates.clone();
    let market_filters = poller.market_filters.clone();
    let account_state = poller.account_state.clone();
    
    let notifier = Arc::new(notifier::TelegramNotifier::new(account_state.clone(), config.clone(), secrets.clone()));
    
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

    // History store (SQLite, 10GB cap) + snapshot channel
    let history_db_path = std::env::var("HISTORY_DB_PATH").unwrap_or_else(|_| "data/arbitro_history.db".to_string());
    let history_store = history::HistoryStore::new(Path::new(&history_db_path)).ok().map(Arc::new);
    let (snapshot_tx, snapshot_rx) = mpsc::channel::<Vec<HistoryOpportunity>>(32);
    let snapshot_tx_for_agg = if let Some(ref store) = history_store {
        let history_writer = store.clone();
        tokio::spawn(async move {
            let mut rx = snapshot_rx;
            while let Some(opportunities) = rx.recv().await {
                if let Err(e) = history_writer.add_snapshot(opportunities) {
                    log::warn!("History write failed: {}", e);
                }
            }
        });
        let history_api_port: u16 = std::env::var("HISTORY_API_PORT").ok().and_then(|s| s.parse().ok()).unwrap_or(8080);
        let history_api_store = store.clone();
        tokio::spawn(async move {
            let app = history::router(history_api_store);
            let addr = std::net::SocketAddr::from(([0, 0, 0, 0], history_api_port));
            if let Err(e) = axum::serve(tokio::net::TcpListener::bind(addr).await.unwrap(), app).await {
                log::warn!("History API server error: {}", e);
            }
        });
        log::info!("History API listening on port {} (DB: {})", history_api_port, history_db_path);
        Some(snapshot_tx)
    } else {
        log::warn!("History store disabled (could not open {}), History tab will be empty", history_db_path);
        drop(snapshot_rx);
        let health_port: u16 = std::env::var("HISTORY_API_PORT").ok().and_then(|s| s.parse().ok()).unwrap_or(8080);
        tokio::spawn(async move {
            let app = history::health_only_router();
            let addr = std::net::SocketAddr::from(([0, 0, 0, 0], health_port));
            let _ = axum::serve(tokio::net::TcpListener::bind(addr).await.unwrap(), app).await;
        });
        None
    };

    // Channels
    let (tx, rx) = mpsc::channel::<UnifiedTicker>(1000);
    let (cmd_tx, cmd_rx) = mpsc::channel::<crate::model::BotCommand>(10);
    let (exec_tx, exec_rx) = mpsc::channel(100);

    // Setup Command Consumer (Botmaster)
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
    let config_snap = config.lock().unwrap_or_else(|e| e.into_inner()).clone();
    exchange::launch_all(tx, &config_snap).await;

    // Run Execution Actor (Background Thread)
    let mut execution_actor = ExecutionActor::new(exec_rx, rate_limiter.clone(), config.clone(), notifier.clone());
    tokio::spawn(async move {
        execution_actor.run().await;
    });

    // Run Aggregator (Main Thread)
    let mut aggregator = Aggregator::new(
        rx,
        cmd_rx,
        exec_tx,
        log_buffer,
        risk_manager,
        rebalance_advisor,
        funding_rates,
        market_filters,
        account_state,
        notifier,
        messaging,
        listener_alert_rx,
        listener_url_tx,
        config,
        snapshot_tx_for_agg,
    );

    // Setup Command Consumer (ArbitHub Internal)
    {
        let msg = aggregator.messaging.lock().await;
        if let Some(client) = msg.as_ref() {
            let consumer = aggregator.get_command_consumer();
            let _ = client.consume("arbit_hub_commands", "commands", consumer).await;
        }
    }

    aggregator.run().await;
}
