use log::{error, info};
use reqwest::Client;
use std::env;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq)]
enum UserState {
    Idle,
    AwaitingPassword,
    AwaitingExchangeSelection,
    AwaitingApiKey(crate::model::ExchangeId),
    AwaitingApiSecret(crate::model::ExchangeId, String), // Exchange, Key
    AwaitingBitgetPassphrase(crate::model::ExchangeId, String, String), // Exchange, Key, Secret
}

pub struct TelegramNotifier {
    client: Client,
    token: String,
    chat_id: Mutex<String>,
    password: String,
    states: Mutex<HashMap<String, UserState>>,
    account_state: Arc<Mutex<crate::model::GlobalAccountState>>,
    enabled: bool,
}

impl TelegramNotifier {
    pub fn new(account_state: Arc<Mutex<crate::model::GlobalAccountState>>) -> Self {
        let token = env::var("TELEGRAM_BOT_TOKEN").unwrap_or_default();
        let chat_id = env::var("TELEGRAM_CHAT_ID").unwrap_or_default();
        // Require explicit password when Telegram is enabled; no insecure default
        let password = env::var("BOT_PASSWORD").unwrap_or_default();
        if !token.is_empty() && password.is_empty() {
            info!("BOT_PASSWORD not set - /login will be disabled until you set it in .env");
        }
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
            account_state,
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
            UserState::AwaitingExchangeSelection => {
                let exchange_opt = match text.to_lowercase().as_str() {
                    "binance" => Some(crate::model::ExchangeId::Binance),
                    "bybit" => Some(crate::model::ExchangeId::Bybit),
                    "bitget" => Some(crate::model::ExchangeId::Bitget),
                    _ => None,
                };

                if let Some(ex) = exchange_opt {
                    {
                        let mut s_map = self.states.lock().unwrap();
                        s_map.insert(chat_id.to_string(), UserState::AwaitingApiKey(ex));
                    }
                    self.send_to_chat(chat_id, &format!("⚙️ *Setup: {}*\nPlease enter your **API Key**:", ex)).await;
                } else {
                   self.send_to_chat(chat_id, "❌ *Invalid Exchange.*\nPlease type: Binance, Bybit, or Bitget (or type /cancel):").await;
                }
            }
            UserState::AwaitingApiKey(ex) => {
                {
                    let mut s_map = self.states.lock().unwrap();
                    s_map.insert(chat_id.to_string(), UserState::AwaitingApiSecret(ex, text.to_string()));
                }
                self.send_to_chat(chat_id, "🔐 *Setup: API Secret*\nPlease enter your **API Secret** (it will NOT be shown in logs):").await;
            }
            UserState::AwaitingApiSecret(ex, key) => {
                if ex == crate::model::ExchangeId::Bitget {
                    {
                        let mut s_map = self.states.lock().unwrap();
                        s_map.insert(chat_id.to_string(), UserState::AwaitingBitgetPassphrase(ex, key, text.to_string()));
                    }
                    self.send_to_chat(chat_id, "🔑 *Setup: Passphrase (Bitget)*\nPlease enter your API Passphrase:").await;
                } else {
                    self.save_credentials(ex, &key, text, "").await;
                    {
                        let mut s_map = self.states.lock().unwrap();
                        s_map.insert(chat_id.to_string(), UserState::Idle);
                    }
                    self.send_to_chat(chat_id, &format!("✅ *Credentials saved for {}!* \nBot will now start polling your private data.", ex)).await;
                }
            }
            UserState::AwaitingBitgetPassphrase(ex, key, secret) => {
                self.save_credentials(ex, &key, &secret, text).await;
                {
                    let mut s_map = self.states.lock().unwrap();
                    s_map.insert(chat_id.to_string(), UserState::Idle);
                }
                self.send_to_chat(chat_id, &format!("✅ *Credentials saved for {}!* \nBot will now start polling your private data.", ex)).await;
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
            if self.password.is_empty() {
                self.send_to_chat(chat_id, "❌ *Login disabled.* Set BOT_PASSWORD in .env first.").await;
                return;
            }
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

        if text.starts_with("/setup") {
            {
                let mut s_map = self.states.lock().unwrap();
                s_map.insert(chat_id.to_string(), UserState::AwaitingExchangeSelection);
            }
            self.send_to_chat(chat_id, "🛠 *API Hookup Wizard*\nWhich exchange do you want to configure?\n\nType: **Binance**, **Bybit**, or **Bitget**").await;
            return;
        }

        if text.starts_with("/cancel") {
            {
                let mut s_map = self.states.lock().unwrap();
                s_map.insert(chat_id.to_string(), UserState::Idle);
            }
            self.send_to_chat(chat_id, "🚫 Setup cancelled.").await;
            return;
        }

        match text {
            "/start" => {
                let msg = format!("👋 *Arbitrage Hub Bot*\n\nAvailable commands:\n/login <pass> - Direct login\n/setup - Configure API Keys\n/status - System status\n/ping - Simple check");
                self.send_to_chat(chat_id, &msg).await;
            }
            "/status" => {
                let msg = {
                    let state_lock = self.account_state.lock().unwrap();
                    let mut exchange_summary = String::new();
                    for (ex, s) in &state_lock.exchange_states {
                        exchange_summary.push_str(&format!("🔹 *{}*: ${:.2} ({} pos)\n", ex, s.total_equity, s.positions.len()));
                    }

                    format!(
                        "📊 *System Status*\n\n\
                        💰 *Total Equity*: `${:.2} USDT`\n\
                        📉 *Global PnL*: `${:.2}`\n\n\
                        *Exchanges*:\n{}\n\
                        Threshold: 5.0%\n\
                        Uptime: Running",
                        state_lock.total_equity_usdt, state_lock.total_unrealized_pnl, exchange_summary
                    )
                };
                self.send_to_chat(chat_id, &msg).await;
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

    /// Appends exchange API credentials to .env so they persist across restarts (sync, used by dashboard path too).
    pub fn append_credentials_to_env(
        exchange: crate::model::ExchangeId,
        key: &str,
        secret: &str,
        passphrase: &str,
    ) {
        use std::io::Write;
        let env_path = ".env";
        let prefix = match exchange {
            crate::model::ExchangeId::Binance => "BINANCE",
            crate::model::ExchangeId::Bybit => "BYBIT",
            crate::model::ExchangeId::Bitget => "BITGET",
            crate::model::ExchangeId::MEXC => "MEXC",
            crate::model::ExchangeId::Okx => "OKX",
            crate::model::ExchangeId::Kraken => "KRAKEN",
            _ => return,
        };

        let key_line = format!("{}_API_KEY={}\n", prefix, key);
        let secret_line = format!("{}_API_SECRET={}\n", prefix, secret);
        let pass_line = if !passphrase.is_empty() {
            format!("{}_API_PASSPHRASE={}\n", prefix, passphrase)
        } else {
            "".to_string()
        };

        if let Ok(mut file) = std::fs::OpenOptions::new().append(true).create(true).open(env_path) {
            let _ = file.write_all(key_line.as_bytes());
            let _ = file.write_all(secret_line.as_bytes());
            if !pass_line.is_empty() {
                let _ = file.write_all(pass_line.as_bytes());
            }
            info!("Saved {} credentials to .env", prefix);
        }
    }

    async fn save_credentials(&self, exchange: crate::model::ExchangeId, key: &str, secret: &str, passphrase: &str) {
        Self::append_credentials_to_env(exchange, key, secret, passphrase);
    }
}
