// /src/services/cache/trades.rs
// Modified: 2025-06-30 14:30:00 EEST

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use std::sync::Arc;
use tracing::{info, debug, error};
use csv::ReaderBuilder;
use std::fs::File;

use crate::{
    db::mysql::MySqlDataSource,
    entities::account::TradingAccount,
    entities::trade::Trade,
    entities::trade_data::TradeDataSource,
    entities::cache::TradesCache,
    logs::app_error::AppError,
};

impl TradesCache {
    // Creates a new TradesCache
    pub fn new(account: Arc<TradingAccount>, pair_id: i32) -> Self {
        debug!("Creating new TradesCache for account_id={}, pair_id={}", account.account_id, pair_id);
        TradesCache {
            data: DashMap::new(),
            account,
            pair_id,
        }
    }

    // Loads trades for a time range from database
    pub async fn load_trades(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<(), AppError> {
        let db = MySqlDataSource::db_conn();
        debug!("Loading trades for account_id={}, pair_id={}, start={}, end={}", 
            self.account.account_id, self.pair_id, start, end);

        let trades = db.get_trades(start, end, &self.account, Some(self.pair_id as u32))
            .await
            .map_err(|e| {
                error!("Failed to fetch trades: {}", e);
                AppError::Internal(format!("Failed to fetch trades: {}", e))
            })?;

        if trades.is_empty() {
            error!("No trades found for account_id={} in range {} to {}", 
                self.account.account_id, start, end);
            return Err(AppError::Internal(format!("No trades found for pair_id={}", self.pair_id)));
        }

        self.data.insert(self.pair_id, trades);
        info!("Loaded {} trades for account_id={}, pair_id={}", 
            self.data.get(&self.pair_id).map(|t| t.len()).unwrap_or(0), 
            self.account.account_id, self.pair_id);
        Ok(())
    }

    // Imports trades from a CSV file
    pub fn import_csv(&self, file_name: String) -> Result<(), AppError> {
        debug!("Importing trades from CSV file: {}", file_name);
        let file = File::open(&file_name)
            .map_err(|e| AppError::Internal(format!("Failed to open CSV file {}: {}", file_name, e)))?;
        let mut rdr = ReaderBuilder::new()
            .has_headers(true) // Assuming header row
            .from_reader(file);

        let mut trades = Vec::new();
        for (i, result) in rdr.records().enumerate() {
            let record = result.map_err(|e| AppError::Internal(format!("Failed to parse CSV record {}: {}", i + 1, e)))?;
            if record.len() != 5 {
                return Err(AppError::Internal(format!("Invalid CSV record {}: expected 5 fields, got {}", i + 1, record.len())));
            }

            let ts_str = record[0].trim();
            debug!("#DBG: Parsing timestamp '{}' for record {}", ts_str, i + 1);

            // Define possible timestamp formats (covering SQL/ClickHouse exports)
            let formats = [
                "%Y-%m-%dT%H:%M:%S%.3fZ",       // 2024-12-01T00:00:00.000Z
                "%Y-%m-%dT%H:%M:%SZ",           // 2024-12-01T00:00:00Z
                "%Y-%m-%d %H:%M:%S%.3f%z",      // 2024-12-01 00:00:00.000+00:00
                "%Y-%m-%d %H:%M:%S%z",          // 2024-12-01 00:00:00+00:00
                "%Y-%m-%dT%H:%M:%S%.3f%z",      // 2024-12-01T00:00:00.000+00:00
                "%Y-%m-%dT%H:%M:%S%z",          // 2024-12-01T00:00:00+00:00
                "%Y-%m-%d %H:%M:%S",            // 2024-12-01 00:00:00
                "%Y-%m-%dT%H:%M:%S",            // 2024-12-01T00:00:00
            ];

            let mut parsed_ts: Option<DateTime<Utc>> = None;
            for &fmt in formats.iter() {
                // Normalize 'Z' to '+00:00' if needed
                let normalized_ts = if ts_str.ends_with('Z') {
                    format!("{}+00:00", ts_str.trim_end_matches('Z'))
                } else {
                    ts_str.to_string()
                };                
                match DateTime::parse_from_str(&normalized_ts, fmt) {
                    Ok(dt) => {
                        parsed_ts = Some(dt.with_timezone(&Utc));                        
                        break;
                    }
                    Err(e) => {                        
                        continue;
                    }
                }
            }

            let ts = parsed_ts.ok_or_else(|| {
                error!("#ERROR: Failed to parse timestamp '{}' in record {}", ts_str, i + 1);
                AppError::Internal(format!("Invalid timestamp in record {}: unable to parse '{}'", i + 1, ts_str))
            })?;

            let buy = match record[1].to_lowercase().as_str() {
                "true" | "1" => true,
                "false" | "0" => false,
                _ => return Err(AppError::Internal(format!("Invalid buy value in record {}: {}", i + 1, record[1].to_string()))),
            };
            let price: f32 = record[2].parse()
                .map_err(|e| AppError::Internal(format!("Invalid price in record {}: {}", i + 1, e)))?;
            let amount: f32 = record[3].parse()
                .map_err(|e| AppError::Internal(format!("Invalid amount in record {}: {}", i + 1, e)))?;
            let trade_no = record[4].to_string();

            trades.push(Trade {
                ts,
                pair_id: self.pair_id,
                buy,
                price,
                amount,
                trade_no,
                order_id: 0,
                position: 0.0,
                rpnl: 0.0,
                flags: 0,
                comission: 0.0,
            });
        }

        if trades.is_empty() {
            error!("No trades imported from CSV file {}", file_name);
            return Err(AppError::Internal(format!("No trades found in CSV file {}", file_name)));
        }

        // Sort trades by ts (primary) and trade_no (secondary)
        trades.sort_by(|a, b| a.ts.cmp(&b.ts).then_with(|| a.trade_no.cmp(&b.trade_no)));

        self.data.insert(self.pair_id, trades);
        info!("Imported {} trades from CSV file {} for pair_id={}", 
            self.data.get(&self.pair_id).map(|t| t.len()).unwrap_or(0), file_name, self.pair_id);
        Ok(())
    }

    // Retrieves trades, loading from database if necessary
    pub async fn get_trades(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<Trade>, AppError> {
        if self.data.get(&self.pair_id).is_none() {
            debug!("Cache miss for pair_id={}, loading trades", self.pair_id);
            self.load_trades(start, end).await?;
        }

        let trades = self.data.get(&self.pair_id)
            .map(|t| t.clone())
            .unwrap_or_default()
            .into_iter()
            .filter(|t| t.ts >= start && t.ts <= end)
            .collect::<Vec<Trade>>();
        
        debug!("Retrieved {} trades for pair_id={}", trades.len(), self.pair_id);
        Ok(trades)
    }

    // Retrieves a trade by trade_no
    pub fn get_trade(&self, trade_no: &String) -> Option<Trade> {
        self.data.get(&self.pair_id)
            .and_then(|trades| trades.iter().find(|t| t.trade_no == *trade_no).cloned())
    }
}