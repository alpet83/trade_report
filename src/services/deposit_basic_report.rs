// /src/services/deposit_basic_report.rs
// Modified: 2025-06-21 14:00:00 EEST

use chrono::{DateTime, Utc};
use crate::{
    entities::account::TradingAccount,
    entities::trade_data::TradeDataSource,
    db::mysql::{MySqlDataSource},
    logs::app_error::AppError,
};

#[derive(serde::Serialize)]
pub struct DepositBasicReport {
    pub start_value: f32,
    pub end_value: f32,
    pub change_percent: f32,
}

pub async fn generate_deposit_report(    
    account: &TradingAccount,
    start_ts: DateTime<Utc>,
    end_ts: DateTime<Utc>,
    value_column: Option<&str>,
) -> Result<DepositBasicReport, AppError> {
    let db = MySqlDataSource::db_conn();
    let history = db.get_funds_history(account, start_ts, end_ts)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to fetch deposit history: {}", e)))?;

    let first = history.first();    
    let last = history.last();
    let start_value = first.map_or(0.0, |d| d.value);
    let end_value = last.map_or(0.0, |d| d.value);
    let change_percent = if start_value != 0.0 {
        ((end_value - start_value) / start_value) * 100.0
    } else {
        100.0
    };

    Ok(DepositBasicReport {        
        start_value,
        end_value,
        change_percent,
    })
}