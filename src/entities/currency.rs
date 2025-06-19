use serde::{Serialize, Deserialize};

// TODO: Implement Currency and DepositState structures
#[derive(Debug, Serialize, Deserialize)]
pub struct Currency {
    pub id: i32,
    pub symbol: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DepositState {
    pub account_id: i32,
    pub currency_id: i32,
    pub balance: f64,
}