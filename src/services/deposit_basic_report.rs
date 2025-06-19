// Modified: 2025-06-19 11:59:00 EEST
// xaiArtifact: artifact_id="36a66a8d-2905-43fc-afd4-e1467c56b1e8", artifact_version_id="2b3c4d5e-6f7a-8b9c-0d1e-2f3a4b5c6d7e"

use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize, Serializer, Deserializer};
use tracing::{info, error, debug};

use crate::{
    entities::{account::TradingAccount, account_data::FundsHistoryRow},
    db::mysql::TradeDataSource,
};

// Custom serialization module for DateTime<Utc> to format as YYYY-MM-DDTHH:MM:SSZ
mod serde_datetime {
    use super::{DateTime, Utc};
    use serde::{Serializer, Deserializer, Serialize, Deserialize};
    use chrono::format::{StrftimeItems, ParseError};

    const FORMAT: &str = "%Y-%m-%dT%H:%M:%SZ";

    pub fn serialize<S>(date: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = date.format(FORMAT).to_string();
        serializer.serialize_str(&s)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        DateTime::parse_from_str(&s, FORMAT)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DepositBasicReport {
    pub account_id: String,
    pub exchange: String,
    #[serde(with = "serde_datetime")]
    pub start_ts: DateTime<Utc>,
    #[serde(with = "serde_datetime")]
    pub end_ts: DateTime<Utc>,
    pub start_value: f32,
    pub end_value: f32,
    pub change_percent: f32,
    pub value_column: String,
}

#[derive(Debug)]
enum ValueColumn {
    Value,
    ValueBtc,
}

impl ValueColumn {
    fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "value_btc" => ValueColumn::ValueBtc,
            _ => ValueColumn::Value,
        }
    }

    fn to_str(&self) -> &str {
        match self {
            ValueColumn::Value => "value",
            ValueColumn::ValueBtc => "value_btc",
        }
    }
}

pub async fn generate_deposit_report(
    db: &dyn TradeDataSource,
    account: &TradingAccount,
    start_ts: DateTime<Utc>,
    end_ts: DateTime<Utc>,
    value_column: Option<&str>,
) -> Result<DepositBasicReport, String> {
    info!("Generating deposit report for account_id={} on {}, value_column={:?}", account.account_id, account.exchange.name, value_column);

    let value_column = value_column.map(ValueColumn::from_str).unwrap_or(ValueColumn::Value);

    debug!("Querying funds history for account_id={} on {} from {} to {}", account.account_id, account.exchange.name.to_lowercase(), start_ts, end_ts);
    let history = db.get_funds_history(
        &account.exchange.name.to_lowercase(),
        account.account_id.parse::<i32>().map_err(|e| format!("Invalid account_id: {}", e))?,
        start_ts,
        end_ts
    )
        .await
        .map_err(|e| format!("Failed to fetch funds history: {}", e))?;

    if history.is_empty() {
        error!("No funds history found for account_id={} on {} from {} to {}", account.account_id, account.exchange.name, start_ts, end_ts);
        return Err("No funds history found".to_string());
    }

    let start_value = history.first().map(|h| match value_column {
        ValueColumn::Value => h.value,
        ValueColumn::ValueBtc => h.value_btc,
    }).unwrap_or(0.0);
    let end_value = history.last().map(|h| match value_column {
        ValueColumn::Value => h.value,
        ValueColumn::ValueBtc => h.value_btc,
    }).unwrap_or(0.0);
    let change_percent = if start_value != 0.0 {
        ((end_value - start_value) / start_value) * 100.0
    } else {
        0.0
    };

    debug!("Report generated: start_value={}, end_value={}, change_percent={}", start_value, end_value, change_percent);

    Ok(DepositBasicReport {
        account_id: account.account_id.clone(),
        exchange: account.exchange.name.clone(),
        start_ts,
        end_ts,
        start_value,
        end_value,
        change_percent,
        value_column: value_column.to_str().to_string(),
    })
}