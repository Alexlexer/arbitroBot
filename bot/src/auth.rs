//! Simple file-based user store for dashboard login/register.
//! Users stored in users.json as { "username": "argon2_hash" }.

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

fn users_file_path() -> String {
    let vol_path = "userdata/users.json";
    if std::path::Path::new("userdata").exists() || std::fs::create_dir_all("userdata").is_ok() {
        vol_path.to_string()
    } else {
        "users.json".to_string()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct UserStore {
    users: HashMap<String, String>, // username -> password_hash
}

pub fn verify_login(username: &str, password: &str) -> bool {
    let store = load_store();
    let hash = match store.users.get(username) {
        Some(h) => h.as_str(),
        None => return false,
    };
    let parsed = match PasswordHash::new(hash) {
        Ok(p) => p,
        Err(_) => return false,
    };
    argon2::Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

pub fn register_user(username: &str, password: &str, invite_code: &str) -> Result<(), String> {
    let expected = std::env::var("INVITE_CODE").unwrap_or_default();
    if expected.is_empty() {
        return Err("Registration is disabled (no INVITE_CODE set)".to_string());
    }
    if invite_code != expected {
        return Err("Invalid invite code".to_string());
    }
    let username = username.trim();
    if username.is_empty() || username.len() > 64 {
        return Err("Invalid username".to_string());
    }
    if password.len() < 8 {
        return Err("Password must be at least 8 characters".to_string());
    }
    let mut store = load_store();
    if store.users.contains_key(username) {
        return Err("Username already taken".to_string());
    }
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| e.to_string())?
        .to_string();
    store.users.insert(username.to_string(), hash);
    save_store(&store)?;
    Ok(())
}

fn load_store() -> UserStore {
    let path = users_file_path();
    if Path::new(&path).exists() {
        if let Ok(data) = fs::read_to_string(&path) {
            if let Ok(store) = serde_json::from_str(&data) {
                return store;
            }
        }
    }
    UserStore::default()
}

fn save_store(store: &UserStore) -> Result<(), String> {
    let path = users_file_path();
    let data = serde_json::to_string_pretty(store).map_err(|e| e.to_string())?;
    fs::write(&path, data).map_err(|e| e.to_string())?;
    Ok(())
}
