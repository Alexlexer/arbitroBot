use crate::model::ExchangeId;
use rust_decimal::Decimal;
use std::env;
use log::{info, error};

pub async fn execute_withdrawal(
    client: &reqwest::Client,
    from: ExchangeId,
    address: &str,
    amount: Decimal,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    info!("WITHDRAW_START: Attempting {} withdrawal of {} USDT to {}", from, amount, address);
    let res = match from {
        ExchangeId::Binance => binance_withdraw(client, address, amount).await,
        ExchangeId::Bybit => bybit_withdraw(client, address, amount).await,
        ExchangeId::MEXC => mexc_withdraw(client, address, amount).await,
        ExchangeId::Okx => okx_withdraw(client, address, amount).await,
        _ => Err(format!("Withdrawal not implemented for {:?}", from).into()),
    };
    
    match &res {
        Ok(id) => info!("WITHDRAW_SUCCESS: {} ID: {}", from, id),
        Err(e) => error!("WITHDRAW_ERROR: {} Error: {}", from, e),
    }
    res
}

async fn binance_withdraw(client: &reqwest::Client, address: &str, amount: Decimal) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let key = env::var("BINANCE_API_KEY")?;
    let secret = env::var("BINANCE_API_SECRET")?;
    let url = "https://api.binance.com/sapi/v1/capital/withdraw/apply";
    let timestamp = chrono::Utc::now().timestamp_millis().to_string();
    
    // TRC20 is usually cheapest/fastest for USDT rebalancing
    let query = format!("coin=USDT&network=TRX&address={}&amount={}&timestamp={}", address, amount, timestamp);
    let sig = hmac_sign(&secret, &query);
    
    let resp = client.post(&format!("{}?{}&signature={}", url, query, sig))
        .header("X-MBX-APIKEY", key)
        .send().await?;
    
    let json: serde_json::Value = resp.json().await?;
    if let Some(id) = json["id"].as_str() {
        Ok(id.to_string())
    } else {
        Err(format!("Binance Withdraw Fail: {}", json).into())
    }
}

async fn bybit_withdraw(client: &reqwest::Client, address: &str, amount: Decimal) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let key = env::var("BYBIT_API_KEY")?;
    let secret = env::var("BYBIT_API_SECRET")?;
    let url = "https://api.bybit.com/v5/asset/withdraw";
    let timestamp = chrono::Utc::now().timestamp_millis().to_string();
    
    let payload = serde_json::json!({
        "coin": "USDT",
        "chain": "TRX",
        "address": address,
        "amount": amount.to_string(),
        "timestamp": timestamp,
        "forceChain": 1
    });
    
    let payload_str = serde_json::to_string(&payload)?;
    let sign_str = format!("{}{}{}5000{}", timestamp, key, "5000", payload_str);
    let sig = hmac_sign(&secret, &sign_str);
    
    info!("Bybit Withdraw Request: URL: {}, Payload: {}, Timestamp: {}, Key: {}, Signature: {}", url, payload_str, timestamp, key, sig);

    let resp = client.post(url)
        .header("X-BAPI-API-KEY", key)
        .header("X-BAPI-TIMESTAMP", timestamp)
        .header("X-BAPI-SIGN", sig)
        .header("X-BAPI-RECV-WINDOW", "5000")
        .json(&payload)
        .send().await?;
    
    let json: serde_json::Value = resp.json().await?;
    if let Some(id) = json["result"]["withdrawId"].as_str() {
        Ok(id.to_string())
    } else {
        Err(format!("Bybit Withdraw Fail: {}", json).into())
    }
}

async fn mexc_withdraw(client: &reqwest::Client, address: &str, amount: Decimal) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let key = env::var("MEXC_API_KEY")?;
    let secret = env::var("MEXC_API_SECRET")?;
    let url = "https://api.mexc.com/api/v3/capital/withdraw/apply";
    let timestamp = chrono::Utc::now().timestamp_millis().to_string();
    
    let query = format!("coin=USDT&network=TRX&address={}&amount={}&timestamp={}", address, amount, timestamp);
    let sig = hmac_sign(&secret, &query);
    
    let resp = client.post(&format!("{}?{}&signature={}", url, query, sig))
        .header("X-MEXC-APIKEY", key)
        .send().await?;
    
    let json: serde_json::Value = resp.json().await?;
    if let Some(id) = json["id"].as_str() {
        Ok(id.to_string())
    } else {
        Err(format!("MEXC Withdraw Fail: {}", json).into())
    }
}

async fn okx_withdraw(client: &reqwest::Client, address: &str, amount: Decimal) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let key = env::var("OKX_API_KEY")?;
    let secret = env::var("OKX_API_SECRET")?;
    let pass = env::var("OKX_API_PASSPHRASE")?;
    let url = "https://www.okx.com/api/v5/asset/withdrawal";
    let timestamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    
    let payload = serde_json::json!({
        "ccy": "USDT",
        "amt": amount.to_string(),
        "dest": "4", // 4: digital currency address
        "toAddr": address,
        "chain": "USDT-TRC20",
        "fee": "1" // Default TRX fee is ~1 USDT
    });
    
    let body = serde_json::to_string(&payload)?;
    let sign_str = format!("{}{}{}{}", timestamp, "POST", "/api/v5/asset/withdrawal", body);
    let sig = okx_sign(&secret, &sign_str);
    
    let resp = client.post(url)
        .header("OK-ACCESS-KEY", key)
        .header("OK-ACCESS-SIGN", sig)
        .header("OK-ACCESS-TIMESTAMP", &timestamp)
        .header("OK-ACCESS-PASSPHRASE", pass)
        .json(&payload)
        .send().await?;
    
    let json: serde_json::Value = resp.json().await?;
    if let Some(data) = json["data"].as_array().and_then(|a| a.first()) {
        if let Some(id) = data["wdId"].as_str() {
            return Ok(id.to_string());
        }
    }
    Err(format!("OKX Withdraw Fail: {}", json).into())
}

fn hmac_sign(secret: &str, payload: &str) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(payload.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

fn okx_sign(secret: &str, payload: &str) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    use base64::{Engine as _, engine::general_purpose};
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(payload.as_bytes());
    general_purpose::STANDARD.encode(mac.finalize().into_bytes())
}
