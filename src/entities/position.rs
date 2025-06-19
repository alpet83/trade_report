use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use sqlx::FromRow;

// TODO: Implement ComplexPosition and PositionHistory structures
#[derive(Serialize, Deserialize, FromRow, Debug)]
pub struct PositionHistory {
    // Placeholder fields
    pub ts: DateTime<Utc>,
    pub pair_id: i32,
    pub account_id: i32,
    pub value: f64,
    pub value_qty: f64,
    pub target: f64,
    pub offset: f64,
}