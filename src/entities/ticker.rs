use serde::{Serialize, Deserialize};

// TODO: Implement TickerInfo structure
#[derive(Serialize, Deserialize, Debug)]
pub struct TickerInfo {
    // Placeholder fields
    pub id: i32,
    pub ticker: String,
    pub symbol: String,
    pub pair_id: i32,
}