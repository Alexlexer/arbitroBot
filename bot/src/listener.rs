use futures_util::{SinkExt, StreamExt};
use log::{error, info};
use serde_json::Value;
use tokio::sync::{mpsc, watch};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use crate::model::ListenerAlert;

/// Connects to Listener WS and forwards `type=alert` messages to the bot via `alert_tx`.
/// - If `ws_url` is empty, the listener client stays disabled.
/// - Supports `type=ping` by replying `{"type":"pong"}`.
pub async fn run_listener_client(
    mut ws_url_rx: watch::Receiver<String>,
    alert_tx: mpsc::Sender<ListenerAlert>,
) {
    let mut current_url;

    loop {
        current_url = ws_url_rx.borrow().clone();
        if current_url.trim().is_empty() {
            // Wait until URL is provided
            if ws_url_rx.changed().await.is_err() {
                return;
            }
            continue;
        }

        info!("Connecting to Listener WS: {}", current_url);
        match connect_async(current_url.clone()).await {
            Ok((ws_stream, _)) => {
                info!("Listener WS connected");
                let (write, read) = ws_stream.split();
                let mut write = write;
                let mut read = read;

                loop {
                    tokio::select! {
                        changed = ws_url_rx.changed() => {
                            if changed.is_err() { return; }
                            // Break and reconnect with new URL
                            break;
                        }
                        msg = read.next() => {
                            let Some(msg) = msg else { break; };
                            match msg {
                                Ok(Message::Text(text)) => {
                                    if let Err(e) = handle_listener_text(&text, &mut write, &alert_tx).await {
                                        error!("Listener message handling error: {}", e);
                                    }
                                }
                                Ok(Message::Binary(_)) => {
                                    // Ignore binary payloads
                                }
                                Ok(Message::Ping(p)) => {
                                    // Tungstenite usually handles pings, but just in case:
                                    let _ = write.send(Message::Pong(p)).await;
                                }
                                Ok(_) => {}
                                Err(e) => {
                                    error!("Listener WS read error: {}", e);
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                error!("Listener WS connect error: {}", e);
            }
        }

        // Reconnect after a short backoff (or sooner if URL changes)
        tokio::select! {
            _ = ws_url_rx.changed() => {}
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(3)) => {}
        }
    }
}

async fn handle_listener_text(
    text: &str,
    write: &mut futures_util::stream::SplitSink<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, Message>,
    alert_tx: &mpsc::Sender<ListenerAlert>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
{
    let v: Value = serde_json::from_str(text)?;
    let msg_type = v.get("type").and_then(|t| t.as_str()).unwrap_or("");

    match msg_type {
        "ping" => {
            // Listener expects pong
            let pong = r#"{"type":"pong"}"#;
            write.send(Message::Text(pong.to_string())).await?;
        }
        "alert" => {
            let symbol = v.get("symbol").and_then(|x| x.as_str()).unwrap_or("").to_string();
            let exchange = v.get("exchange").and_then(|x| x.as_str()).unwrap_or("").to_string();
            let kind = v.get("kind").and_then(|x| x.as_str()).unwrap_or("").to_string();
            let source = v.get("source").and_then(|x| x.as_str()).unwrap_or("").to_string();
            let reason = v.get("reason").and_then(|x| x.as_str()).unwrap_or("").to_string();
            let at = v.get("at").and_then(|x| x.as_i64()).unwrap_or_else(|| chrono::Utc::now().timestamp_millis());
            let mcap_usd = v.get("mcap_usd").and_then(|x| x.as_f64());

            if !symbol.is_empty() {
                let alert = ListenerAlert {
                    symbol,
                    exchange,
                    kind,
                    source,
                    reason,
                    at,
                    mcap_usd,
                };
                let _ = alert_tx.send(alert).await;
            }
        }
        _ => {}
    }

    Ok(())
}

