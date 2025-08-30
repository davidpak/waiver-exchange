use serde_json::Value;
use std::fs;
use std::io::{BufRead, Write};
use std::path::PathBuf;
use whistle::{OrderType, Side};

pub struct SessionCommunication {
    session_dir: PathBuf,
    account_id: u32,
}

impl SessionCommunication {
    pub fn new(session_name: &str, account_id: u32) -> Result<Self, String> {
        let sessions_dir = std::env::temp_dir().join("whistle-exchange");
        let session_dir = sessions_dir.join(session_name);

        if !session_dir.exists() {
            return Err(format!("Session '{}' does not exist", session_name));
        }

        Ok(Self { session_dir, account_id })
    }

    pub fn submit_order(
        &self,
        order_id: u64,
        side: Side,
        order_type: OrderType,
        price: Option<u32>,
        qty: u32,
        timestamp: u64,
    ) -> Result<(), String> {
        let order_file = self.session_dir.join("orders.jsonl");
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(order_file)
            .map_err(|e| format!("Failed to open orders file: {}", e))?;

        let order_data = serde_json::json!({
            "account_id": self.account_id,
            "order_id": order_id,
            "side": match side {
                Side::Buy => "buy",
                Side::Sell => "sell",
            },
            "order_type": match order_type {
                OrderType::Limit => "limit",
                OrderType::Market => "market",
                OrderType::Ioc => "ioc",
                OrderType::PostOnly => "post_only",
            },
            "price": price,
            "qty": qty,
            "timestamp": timestamp,
        });

        writeln!(file, "{}", order_data.to_string())
            .map_err(|e| format!("Failed to write order: {}", e))?;

        Ok(())
    }

    pub fn read_responses(&self) -> Result<Vec<Value>, String> {
        let response_file = self.session_dir.join("responses.jsonl");
        if !response_file.exists() {
            return Ok(Vec::new());
        }

        let file = fs::File::open(response_file)
            .map_err(|e| format!("Failed to open responses file: {}", e))?;
        let reader = std::io::BufReader::new(file);
        let mut responses = Vec::new();

        for line in reader.lines() {
            let line = line.map_err(|e| format!("Failed to read response line: {}", e))?;
            if !line.trim().is_empty() {
                let response: Value = serde_json::from_str(&line)
                    .map_err(|e| format!("Failed to parse response JSON: {}", e))?;
                responses.push(response);
            }
        }

        Ok(responses)
    }

    pub fn read_trades(&self) -> Result<Vec<Value>, String> {
        let trade_file = self.session_dir.join("trades.jsonl");
        if !trade_file.exists() {
            return Ok(Vec::new());
        }

        let file =
            fs::File::open(trade_file).map_err(|e| format!("Failed to open trades file: {}", e))?;
        let reader = std::io::BufReader::new(file);
        let mut trades = Vec::new();

        for line in reader.lines() {
            let line = line.map_err(|e| format!("Failed to read trade line: {}", e))?;
            if !line.trim().is_empty() {
                let trade: Value = serde_json::from_str(&line)
                    .map_err(|e| format!("Failed to parse trade JSON: {}", e))?;
                trades.push(trade);
            }
        }

        Ok(trades)
    }

    pub fn read_book_updates(&self) -> Result<Vec<Value>, String> {
        let book_file = self.session_dir.join("book_updates.jsonl");
        if !book_file.exists() {
            return Ok(Vec::new());
        }

        let file = fs::File::open(book_file)
            .map_err(|e| format!("Failed to open book updates file: {}", e))?;
        let reader = std::io::BufReader::new(file);
        let mut updates = Vec::new();

        for line in reader.lines() {
            let line = line.map_err(|e| format!("Failed to read book update line: {}", e))?;
            if !line.trim().is_empty() {
                let update: Value = serde_json::from_str(&line)
                    .map_err(|e| format!("Failed to parse book update JSON: {}", e))?;
                updates.push(update);
            }
        }

        Ok(updates)
    }

    pub fn clear_files(&self) -> Result<(), String> {
        let files = ["orders.jsonl", "responses.jsonl", "trades.jsonl", "book_updates.jsonl"];
        for file in files {
            let file_path = self.session_dir.join(file);
            if file_path.exists() {
                std::fs::remove_file(&file_path)
                    .map_err(|e| format!("Failed to remove {}: {}", file, e))?;
            }
        }
        Ok(())
    }
}
