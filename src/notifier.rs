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
    secrets: Arc<Mutex<crate::config::SecretsConfig>>,
    password: String,
    states: Mutex<HashMap<String, UserState>>,
    account_state: Arc<Mutex<crate::model::GlobalAccountState>>,
    config: Arc<Mutex<crate::config::AppConfig>>,
}

impl TelegramNotifier {
    pub fn new(account_state: Arc<Mutex<crate::model::GlobalAccountState>>) -> Self {
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
            secrets,
            password,
            states: Mutex::new(HashMap::new()),
            account_state,
            config,
        }
    }

    pub async fn send_alert(&self, message: &str) {
        let cid = {
            let sec = self.secrets.lock().unwrap();
            sec.telegram_chat_id.clone().unwrap_or_default()
        };
        if cid.is_empty() { return; }
        self.send_to_chat(&cid, message, None).await;
    }

    pub async fn send_rebalance_confirmation(&self, advice: &crate::model::RebalanceAdvice) {

        // Rebalance specific enablement check
        {
            let conf = self.config.lock().unwrap();
            if !conf.automated_rebalance_enabled {
                return;
            }
        }
        let cid = {
            let sec = self.secrets.lock().unwrap();
            sec.telegram_chat_id.clone().unwrap_or_default()
        };
        if cid.is_empty() { return; }

        let message = format!(
            "🚨 *REBALANCE ADVICE*\n\n\
            From: *{}*\n\
            To: *{}*\n\
            Amount: `${:.2} USDT`\n\
            Reason: _{}_",
            advice.from_exchange, advice.to_exchange, advice.amount_usdt, advice.reason
        );

        let callback_data = format!("rebalance_exec:{}:{}:{}", advice.from_exchange, advice.to_exchange, advice.amount_usdt);
        let keyboard = serde_json::json!({
            "inline_keyboard": [[
                { "text": "✅ Execute", "callback_data": callback_data },
                { "text": "❌ Cancel", "callback_data": "rebalance_cancel" }
            ]]
        });

        self.send_to_chat(&cid, &message, Some(keyboard)).await;
    }

    async fn send_to_chat(&self, chat_id: &str, message: &str, reply_markup: Option<serde_json::Value>) {
        let token = {
            let sec = self.secrets.lock().unwrap();
            sec.telegram_token.clone().unwrap_or_default()
        };
        if token.is_empty() { return; }

        let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
        let mut params = vec![
            ("chat_id".to_string(), chat_id.to_string()),
            ("text".to_string(), message.to_string()),
            ("parse_mode".to_string(), "Markdown".to_string()),
        ];

        if let Some(markup) = reply_markup {
            params.push(("reply_markup".to_string(), markup.to_string()));
        }

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
        
        info!("Starting Telegram Command Listener...");
        let mut offset = 0;

        loop {
            let token = {
                let sec = self.secrets.lock().unwrap();
                sec.telegram_token.clone().unwrap_or_default()
            };
            if token.is_empty() {
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                continue;
            }

            let url = format!("https://api.telegram.org/bot{}/getUpdates?offset={}&timeout=30", token, offset);
            
            match self.client.get(&url).send().await {
                Ok(resp) => {
                    if let Ok(json) = resp.json::<serde_json::Value>().await {
                        if let Some(updates) = json["result"].as_array() {
                            for update in updates {
                                if let Some(update_id) = update["update_id"].as_i64() {
                                    offset = update_id + 1;
                                }

                                if let Some(callback) = update["callback_query"].as_object() {
                                    let chat_id = callback["message"]["chat"]["id"].to_string();
                                    let data = callback["data"].as_str().unwrap_or("");
                                    let callback_id = callback["id"].as_str().unwrap_or("");

                                    self.handle_callback(&chat_id, data, callback_id).await;
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
                        let mut sec = self.secrets.lock().unwrap();
                        sec.telegram_chat_id = Some(chat_id.to_string());
                        let _ = sec.save();
                    }
                    {
                        let mut s_map = self.states.lock().unwrap();
                        s_map.insert(chat_id.to_string(), UserState::Idle);
                    }
                    self.send_to_chat(chat_id, "✅ *Login Successful!*\nYou are now the authorized user for this bot. Alerts will be sent here.", None).await;
                } else {
                    self.send_to_chat(chat_id, "❌ *Incorrect Password.*\nPlease try again or use /login to restart.", None).await;
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
                    self.send_to_chat(chat_id, &format!("⚙️ *Setup: {}*\nPlease enter your **API Key**:", ex), None).await;
                } else {
                   self.send_to_chat(chat_id, "❌ *Invalid Exchange.*\nPlease type: Binance, Bybit, or Bitget (or type /cancel):", None).await;
                }
            }
            UserState::AwaitingApiKey(ex) => {
                {
                    let mut s_map = self.states.lock().unwrap();
                    s_map.insert(chat_id.to_string(), UserState::AwaitingApiSecret(ex, text.to_string()));
                }
                self.send_to_chat(chat_id, "🔐 *Setup: API Secret*\nPlease enter your **API Secret** (it will NOT be shown in logs):", None).await;
            }
            UserState::AwaitingApiSecret(ex, key) => {
                if ex == crate::model::ExchangeId::Bitget {
                    {
                        let mut s_map = self.states.lock().unwrap();
                        s_map.insert(chat_id.to_string(), UserState::AwaitingBitgetPassphrase(ex, key, text.to_string()));
                    }
                    self.send_to_chat(chat_id, "🔑 *Setup: Passphrase (Bitget)*\nPlease enter your API Passphrase:", None).await;
                } else {
                    self.save_credentials(ex, &key, text, "").await;
                    {
                        let mut s_map = self.states.lock().unwrap();
                        s_map.insert(chat_id.to_string(), UserState::Idle);
                    }
                    self.send_to_chat(chat_id, &format!("✅ *Credentials saved for {}!* \nBot will now start polling your private data.", ex), None).await;
                }
            }
            UserState::AwaitingBitgetPassphrase(ex, key, secret) => {
                self.save_credentials(ex, &key, &secret, text).await;
                {
                    let mut s_map = self.states.lock().unwrap();
                    s_map.insert(chat_id.to_string(), UserState::Idle);
                }
                self.send_to_chat(chat_id, &format!("✅ *Credentials saved for {}!* \nBot will now start polling your private data.", ex), None).await;
            }
            UserState::Idle => {
                self.handle_command(chat_id, text).await;
            }
        }
    }

    async fn handle_command(&self, chat_id: &str, text: &str) {
        let authorized_cid = {
            let sec = self.secrets.lock().unwrap();
            sec.telegram_chat_id.clone().unwrap_or_default()
        };

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
                        let mut sec = self.secrets.lock().unwrap();
                        sec.telegram_chat_id = Some(chat_id.to_string());
                        let _ = sec.save();
                    }
                    self.send_to_chat(chat_id, "✅ *Login Successful!*\nYou provided the correct password. You are now authorized.", None).await;
                } else {
                    self.send_to_chat(chat_id, "❌ *Incorrect Password.*", None).await;
                }
            } else {
                // Multi-step login: /login
                {
                    let mut s_map = self.states.lock().unwrap();
                    s_map.insert(chat_id.to_string(), UserState::AwaitingPassword);
                }
                self.send_to_chat(chat_id, "🔐 *Authentication Required*\nPlease enter the Access Password:", None).await;
            }
            return;
        }

        if text.starts_with("/setup") {
            {
                let mut s_map = self.states.lock().unwrap();
                s_map.insert(chat_id.to_string(), UserState::AwaitingExchangeSelection);
            }
            self.send_to_chat(chat_id, "🛠 *API Hookup Wizard*\nWhich exchange do you want to configure?\n\nType: **Binance**, **Bybit**, or **Bitget**", None).await;
            return;
        }

        if text.starts_with("/cancel") {
            {
                let mut s_map = self.states.lock().unwrap();
                s_map.insert(chat_id.to_string(), UserState::Idle);
            }
            self.send_to_chat(chat_id, "🚫 Setup cancelled.", None).await;
            return;
        }

        match text {
            "/start" => {
                let msg = format!("👋 *Arbitrage Hub Bot*\n\nAvailable commands:\n/login <pass> - Direct login\n/setup - Configure API Keys\n/status - System status\n/ping - Simple check");
                self.send_to_chat(chat_id, &msg, None).await;
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
                self.send_to_chat(chat_id, &msg, None).await;
            }
            "/ping" => {
                self.send_to_chat(chat_id, "🏓 Pong!", None).await;
            }
            _ => {
                if text.starts_with('/') {
                    self.send_to_chat(chat_id, "❓ Unknown command. Try /start", None).await;
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
        }
    }

    async fn handle_callback(&self, chat_id: &str, data: &str, callback_id: &str) {
        if data == "rebalance_cancel" {
            let _ = self.answer_callback(callback_id, "Rebalance cancelled.").await;
            let _ = self.send_to_chat(chat_id, "🚫 *Rebalance cancelled* by user.", None).await;
            return;
        }

        if data.starts_with("rebalance_exec:") {
            let parts: Vec<&str> = data.split(':').collect();
            if parts.len() == 4 {
                let from_str = parts[1];
                let to_str = parts[2];
                let amount_str = parts[3];
                
                let _ = self.answer_callback(callback_id, "Executing transfer...").await;
                let _ = self.send_to_chat(chat_id, &format!("⏳ *Executing transfer* of `${}` from *{}* to *{}*...", amount_str, from_str, to_str), None).await;
                
                info!("USER AUTHORIZED REBALANCE: From {} to {} Amount {}", from_str, to_str, amount_str);

                // Actual execution logic
                let from_eid = match from_str.to_lowercase().as_str() {
                    "binance" => crate::model::ExchangeId::Binance,
                    "bybit" => crate::model::ExchangeId::Bybit,
                    "mexc" => crate::model::ExchangeId::MEXC,
                    "okx" => crate::model::ExchangeId::Okx,
                    _ => {
                        let _ = self.send_to_chat(chat_id, "❌ Error: Unsupported exchange.", None).await;
                        return;
                    }
                };

                let to_eid = match to_str.to_lowercase().as_str() {
                    "binance" => crate::model::ExchangeId::Binance,
                    "bybit" => crate::model::ExchangeId::Bybit,
                    "mexc" => crate::model::ExchangeId::MEXC,
                    "okx" => crate::model::ExchangeId::Okx,
                    _ => {
                        let _ = self.send_to_chat(chat_id, "❌ Error: Unsupported target exchange.", None).await;
                        return;
                    }
                };

                let amount: rust_decimal::Decimal = amount_str.parse().unwrap_or_default();
                
                let address = {
                    let conf = self.config.lock().unwrap();
                    conf.wallets.get(&to_eid).cloned()
                };

                if let Some(addr) = address {
                    if addr.contains("YOUR_") {
                        let _ = self.send_to_chat(chat_id, "❌ *Error*: Wallet address not configured in `config.json`.", None).await;
                        return;
                    }
                    
                    let token_opt = {
                        let sec = self.secrets.lock().unwrap();
                        sec.telegram_token.clone()
                    };
                    let t_cid = chat_id.to_string();
                    let client = self.client.clone();
                    
                    if let Some(token_clone) = token_opt {
                        let addr_owned = addr.clone();
                        let to_str_owned = to_str.to_string();

                        tokio::spawn(async move {
                            match crate::transfers::execute_withdrawal(&client, from_eid, &addr_owned, amount).await {
                                Ok(id) => {
                                    let url = format!("https://api.telegram.org/bot{}/sendMessage", token_clone);
                                    let message = format!("✅ *Transfer Successful!*\nID: `{}`\nFunds are on their way to {}.", id, to_str_owned);
                                    let _ = reqwest::Client::new().post(&url)
                                        .form(&[("chat_id", &t_cid), ("text", &message), ("parse_mode", &"Markdown".to_string())])
                                        .send().await;
                                }
                                Err(e) => {
                                    let url = format!("https://api.telegram.org/bot{}/sendMessage", token_clone);
                                    let message = format!("❌ *Transfer Failed*\nError: `{}`", e);
                                    let _ = reqwest::Client::new().post(&url)
                                        .form(&[("chat_id", &t_cid), ("text", &message), ("parse_mode", &"Markdown".to_string())])
                                        .send().await;
                                }
                            }
                        });
                    }
                } else {
                    let _ = self.send_to_chat(chat_id, &format!("❌ *Error*: No wallet address found for {} in config.", to_str), None).await;
                }
            }
        }
    }

    async fn answer_callback(&self, callback_id: &str, text: &str) -> Result<(), Box<dyn std::error::Error>> {
        let token = {
            let sec = self.secrets.lock().unwrap();
            sec.telegram_token.clone().unwrap_or_default()
        };
        if token.is_empty() { return Ok(()); }

        let url = format!("https://api.telegram.org/bot{}/answerCallbackQuery", token);
        let params = [
            ("callback_query_id", callback_id.to_string()),
            ("text", text.to_string()),
        ];
        self.client.post(&url).form(&params).send().await?;
        Ok(())
    }

    async fn save_credentials(&self, exchange: crate::model::ExchangeId, key: &str, secret: &str, passphrase: &str) {
        Self::append_credentials_to_env(exchange, key, secret, passphrase);
    }
}
