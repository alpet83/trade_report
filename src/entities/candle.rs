use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

// TODO: Implement Candle and Tick structures
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Candle {
    // Placeholder fields
    pub ts: DateTime<Utc>,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Tick {
    // Placeholder fields
    pub ts: DateTime<Utc>,
    pub price: f64,
    pub volume: f64,
}