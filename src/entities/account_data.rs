// Modified: 2025-06-19 12:45:00 EEST
// xaiArtifact: artifact_id="974ab47c-c84f-43ef-b9cf-8ad071ecee12", artifact_version_id="5f6a7b8c-9d0e-1f2a-3b4c-5d6e7f8a9b0c"

use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct FundsHistoryRow {
    pub ts: DateTime<Utc>,
    pub value: f32,
    pub value_btc: f32,
    pub position_coef: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DepositHistoryRow {
    pub ts: DateTime<Utc>,
    pub withdrawal: bool,
    pub value_usd: f32,
    pub value_btc: f32,
}