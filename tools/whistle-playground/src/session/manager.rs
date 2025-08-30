use crate::session::config::{AccountType, ParticipantInfo, SessionConfig};
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use whistle::{OrderType, Side};

pub struct SessionManager {
    sessions_dir: PathBuf,
}

impl SessionManager {
    pub fn new() -> Self {
        let sessions_dir = std::env::temp_dir().join("whistle-exchange");
        fs::create_dir_all(&sessions_dir).expect("Failed to create sessions directory");
        Self { sessions_dir }
    }

    pub fn create_session(&self, name: &str, accounts: u32) -> Result<SessionConfig, String> {
        let session_dir = self.sessions_dir.join(name);

        if session_dir.exists() {
            return Err(format!("Session '{}' already exists", name));
        }

        fs::create_dir_all(&session_dir)
            .map_err(|e| format!("Failed to create session directory: {}", e))?;

        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

        let mut participants = HashMap::new();
        for i in 1..=accounts {
            let account_type = match i {
                1 | 5 => AccountType::MarketMaker,
                2 | 3 => AccountType::Trader,
                4 => AccountType::Institutional,
                _ => AccountType::Trader,
            };

            let name = match account_type {
                AccountType::MarketMaker => format!("market_maker_{}", if i == 1 { 1 } else { 2 }),
                AccountType::Trader => format!("trader_{}", i),
                AccountType::Institutional => "institutional".to_string(),
            };

            participants
                .insert(i, ParticipantInfo { name, account_type, connected: false, last_seen: 0 });
        }

        let config = SessionConfig {
            name: name.to_string(),
            accounts,
            created: now,
            last_activity: now,
            participants,
        };

        self.save_session_config(&config)?;
        self.create_session_files(name)?;

        Ok(config)
    }

    pub fn join_session(&self, name: &str, account: u32) -> Result<SessionConfig, String> {
        let mut config = self.load_session_config(name)?;

        if account < 1 || account > config.accounts {
            return Err(format!(
                "Account {} is not valid for session '{}' (1-{})",
                account, name, config.accounts
            ));
        }

        if let Some(participant) = config.participants.get_mut(&account) {
            participant.connected = true;
            participant.last_seen = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        }

        self.save_session_config(&config)?;
        self.create_pid_file(name, account)?;

        Ok(config)
    }

    pub fn join_session_with_type(
        &self,
        name: &str,
        account: u32,
        account_type: &str,
    ) -> Result<SessionConfig, String> {
        let mut config = self.load_session_config(name)?;

        if account < 1 || account > config.accounts {
            return Err(format!(
                "Account {} is not valid for session '{}' (1-{})",
                account, name, config.accounts
            ));
        }

        let account_type_enum = match account_type.to_lowercase().as_str() {
            "marketmaker" | "market_maker" => AccountType::MarketMaker,
            "trader" => AccountType::Trader,
            "institutional" => AccountType::Institutional,
            _ => return Err(format!("Invalid account type: {}", account_type)),
        };

        if let Some(participant) = config.participants.get_mut(&account) {
            participant.connected = true;
            participant.last_seen = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
            participant.account_type = account_type_enum;
        }

        self.save_session_config(&config)?;
        self.create_pid_file(name, account)?;

        Ok(config)
    }

    pub fn list_sessions(&self) -> Vec<SessionConfig> {
        let mut sessions = Vec::new();

        if let Ok(entries) = fs::read_dir(&self.sessions_dir) {
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();

                    if path.is_dir() {
                        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                            if let Ok(config) = self.load_session_config(name) {
                                sessions.push(config);
                            }
                        }
                    }
                }
            }
        }

        sessions
    }

    pub fn session_exists(&self, name: &str) -> bool {
        self.sessions_dir.join(name).exists()
    }

    pub fn get_session_info(&self, name: &str) -> Option<SessionConfig> {
        self.load_session_config(name).ok()
    }

    pub fn cleanup_expired_sessions(&mut self) -> usize {
        let mut removed = 0;
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let expiry_time = 24 * 60 * 60; // 24 hours

        let sessions = self.list_sessions();
        for session in sessions {
            if now - session.last_activity > expiry_time {
                if let Ok(_) = fs::remove_dir_all(self.sessions_dir.join(&session.name)) {
                    removed += 1;
                }
            }
        }

        removed
    }

    pub fn submit_order_to_session(
        &self,
        session_name: &str,
        account_id: u32,
        order_id: u64,
        side: Side,
        order_type: OrderType,
        price: Option<u32>,
        qty: u32,
    ) -> Result<(), String> {
        let session_dir = self.sessions_dir.join(session_name);
        if !session_dir.exists() {
            return Err(format!("Session '{}' does not exist", session_name));
        }

        let order_file = session_dir.join("orders.jsonl");
        let order_data = json!({
            "account_id": account_id,
            "order_id": order_id,
            "side": if side == Side::Buy { "buy" } else { "sell" },
            "order_type": match order_type {
                OrderType::Limit => "limit",
                OrderType::Market => "market",
                OrderType::Ioc => "ioc",
                OrderType::PostOnly => "post-only",
            },
            "price": price,
            "qty": qty,
            "timestamp": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis(),
        });

        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(order_file)
            .map_err(|e| format!("Failed to open orders file: {}", e))?;

        use std::io::Write;
        writeln!(file, "{}", order_data.to_string())
            .map_err(|e| format!("Failed to write order: {}", e))?;

        // Update session activity
        if let Ok(mut config) = self.load_session_config(session_name) {
            config.last_activity = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
            if let Some(participant) = config.participants.get_mut(&account_id) {
                participant.last_seen = config.last_activity;
            }
            let _ = self.save_session_config(&config);
        }

        Ok(())
    }

    fn load_session_config(&self, name: &str) -> Result<SessionConfig, String> {
        let config_file = self.sessions_dir.join(name).join("config.json");
        let content = fs::read_to_string(config_file)
            .map_err(|e| format!("Failed to read session config: {}", e))?;

        serde_json::from_str(&content).map_err(|e| format!("Failed to parse session config: {}", e))
    }

    fn save_session_config(&self, config: &SessionConfig) -> Result<(), String> {
        let config_file = self.sessions_dir.join(&config.name).join("config.json");
        let content = serde_json::to_string_pretty(config)
            .map_err(|e| format!("Failed to serialize session config: {}", e))?;

        fs::write(config_file, content)
            .map_err(|e| format!("Failed to write session config: {}", e))
    }

    fn create_session_files(&self, name: &str) -> Result<(), String> {
        let session_dir = self.sessions_dir.join(name);

        // Create communication files
        let files = ["orders.jsonl", "responses.jsonl", "trades.jsonl", "book_updates.jsonl"];
        for file in files {
            let file_path = session_dir.join(file);
            fs::write(file_path, "").map_err(|e| format!("Failed to create {}: {}", file, e))?;
        }

        Ok(())
    }

    fn create_pid_file(&self, name: &str, account: u32) -> Result<(), String> {
        let pid_file = self.sessions_dir.join(name).join(format!("account_{}.pid", account));
        let pid = std::process::id();
        fs::write(pid_file, pid.to_string())
            .map_err(|e| format!("Failed to create PID file: {}", e))
    }
}
