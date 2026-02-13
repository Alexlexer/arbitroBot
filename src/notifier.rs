use log::{error, info};
use reqwest::Client;
use std::env;
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Debug, Clone, PartialEq)]
enum UserState {
    Idle,
    AwaitingPassword,
}

pub struct TelegramNotifier {
    client: Client,
    token: String,
    chat_id: Mutex<String>,
    password: String,
    states: Mutex<HashMap<String, UserState>>,
    enabled: bool,
}

impl TelegramNotifier {
    pub fn new() -> Self {
        let token = env::var("TELEGRAM_BOT_TOKEN").unwrap_or_default();
        let chat_id = env::var("TELEGRAM_CHAT_ID").unwrap_or_default();
        let password = env::var("BOT_PASSWORD").unwrap_or_else(|_| "admin123".to_string());
        let enabled = !token.is_empty();

        if token.is_empty() {
            info!("Telegram Notifications are DISABLED (Missing Token)");
        } else if chat_id.is_empty() {
             info!("Telegram BOT TOKEN found, but CHAT_ID is missing. Use /login to authenticate.");
        }

        Self {
            client: Client::new(),
            token,
            chat_id: Mutex::new(chat_id),
            password,
            states: Mutex::new(HashMap::new()),
            enabled,
        }
    }

    pub async fn send_alert(&self, message: &str) {
        if !self.enabled {
            return;
        }
        let cid = self.chat_id.lock().unwrap().clone();
        if cid.is_empty() {
            return;
        }
        self.send_to_chat(&cid, message).await;
    }

    async fn send_to_chat(&self, chat_id: &str, message: &str) {
        let url = format!("https://api.telegram.org/bot{}/sendMessage", self.token);
        let params = [
            ("chat_id", &chat_id.to_string()),
            ("text", &message.to_string()),
            ("parse_mode", &"Markdown".to_string()),
        ];

        match self.client.post(&url).form(&params).send().await {
            Ok(resp) => {
                if !resp.status().is_success() {
                    let err_text = resp.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                    error!("Failed to send Telegram message: {}", err_text);
                }
            }
            Err(e) => {
                error!("Network error sending Telegram message: {}", e);
            }
        }
    }

    pub async fn run_listener(&self) {
        if !self.enabled { return; }
        
        info!("Starting Telegram Command Listener...");
        let mut offset = 0;

        loop {
            let url = format!("https://api.telegram.org/bot{}/getUpdates?offset={}&timeout=30", self.token, offset);
            
            match self.client.get(&url).send().await {
                Ok(resp) => {
                    if let Ok(json) = resp.json::<serde_json::Value>().await {
                        if let Some(updates) = json["result"].as_array() {
                            for update in updates {
                                if let Some(update_id) = update["update_id"].as_i64() {
                                    offset = update_id + 1;
                                }

                                if let Some(message) = update["message"].as_object() {
                                    let chat_id = message["chat"]["id"].to_string();
                                    let text = message["text"].as_str().unwrap_or("");

                                    self.handle_message(&chat_id, text).await;
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Error polling Telegram updates: {}", e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }
            }
        }
    }

    async fn handle_message(&self, chat_id: &str, text: &str) {
        let state = {
            let s_map = self.states.lock().unwrap();
            s_map.get(chat_id).cloned().unwrap_or(UserState::Idle)
        };

        match state {
            UserState::AwaitingPassword => {
                if text == self.password {
                    {
                        let mut cid = self.chat_id.lock().unwrap();
                        *cid = chat_id.to_string();
                    }
                    {
                        let mut s_map = self.states.lock().unwrap();
                        s_map.insert(chat_id.to_string(), UserState::Idle);
                    }
                    self.send_to_chat(chat_id, "✅ *Login Successful!*\nYou are now the authorized user for this bot. Alerts will be sent here.").await;
                } else {
                    self.send_to_chat(chat_id, "❌ *Incorrect Password.*\nPlease try again or use /login to restart.").await;
                    {
                        let mut s_map = self.states.lock().unwrap();
                        s_map.insert(chat_id.to_string(), UserState::Idle);
                    }
                }
            }
            UserState::Idle => {
                self.handle_command(chat_id, text).await;
            }
        }
    }

    async fn handle_command(&self, chat_id: &str, text: &str) {
        let authorized_cid = self.chat_id.lock().unwrap().clone();

        // Security check for regular commands
        if !authorized_cid.is_empty() && chat_id != authorized_cid {
            if text.starts_with('/') && !text.starts_with("/login") {
                info!("Unauthorized command from ChatID {}: {}", chat_id, text);
                return;
            }
        }

        if text.starts_with("/login") {
            let parts: Vec<&str> = text.split_whitespace().collect();
            
            if parts.len() > 1 {
                // Direct login: /login <password>
                let input_pass = parts[1];
                if input_pass == self.password {
                    {
                        let mut cid = self.chat_id.lock().unwrap();
                        *cid = chat_id.to_string();
                    }
                    self.send_to_chat(chat_id, "✅ *Login Successful!*\nYou provided the correct password. You are now authorized.").await;
                } else {
                    self.send_to_chat(chat_id, "❌ *Incorrect Password.*").await;
                }
            } else {
                // Multi-step login: /login
                {
                    let mut s_map = self.states.lock().unwrap();
                    s_map.insert(chat_id.to_string(), UserState::AwaitingPassword);
                }
                self.send_to_chat(chat_id, "🔐 *Authentication Required*\nPlease enter the Access Password:").await;
            }
            return;
        }

        match text {
            "/start" => {
                let msg = format!("👋 *Arbitrage Hub Bot*\n\nAvailable commands:\n/login <pass> - Direct login\n/status - System status\n/ping - Simple check");
                self.send_to_chat(chat_id, &msg).await;
            }
            "/status" => {
                let msg = "📊 *System Status*\n\nUptime: Running\nThreshold: 5.0%\nExchanges: 3/3";
                self.send_to_chat(chat_id, msg).await;
            }
            "/ping" => {
                self.send_to_chat(chat_id, "🏓 Pong!").await;
            }
            _ => {
                if text.starts_with('/') {
                    self.send_to_chat(chat_id, "❓ Unknown command. Try /start").await;
                }
            }
        }
    }
}
