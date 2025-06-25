use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use sqlx::FromRow;

#[derive(Serialize, Deserialize, FromRow, Debug, Clone)]
pub struct Trade {
    pub ts: DateTime<Utc>,
    pub pair_id: i32,
    pub buy: bool,
    pub price: f32,
    pub amount: f32,
    pub trade_no: String,
    pub order_id: u32,
    pub position: f32,
    pub rpnl: f32,
    pub flags: i32,
    pub comission: f32,
}

#[derive(Serialize, Deserialize, FromRow, Debug, Clone)]
pub struct Order {
    pub ts: DateTime<Utc>,
    pub pair_id: i32,
    pub buy: bool,
    pub order_id: String,
    pub status: String,
    pub price: f64,
    pub amount: f64,
}

#[derive(Serialize, Deserialize, FromRow, Debug, Clone)]
pub struct OrdersBatch {
    pub id: i32,    
    pub pair_id: i32,
    pub ts: DateTime<Utc>,
    pub parent: i32,
    pub source_pos: Option<f32>,
    pub start_pos: f32,
    pub target_pos: f64,
    pub price: f32,
    pub exec_price: f32,
    pub btc_price: Option<f32>,
    pub exec_amount: f32,
    pub exec_qty: f32,
    pub slippage: f32,
    pub last_order: u32,
    pub flags: i32,
}

#[derive(Serialize, Deserialize, FromRow, Debug, Clone)]
pub struct TradeSignal {
    pub id: i32,    
    pub buy: bool,
    pub pair_id: i32,
    pub ts: DateTime<Utc>,
    pub ts_checked: DateTime<Utc>,
    pub limit_price: f64,
    pub recalc_price: f64,
    pub stop_loss: f64,
    pub take_profit: f64,
    pub take_order: i32,
    pub limit_order: i32,
    pub amount: i32,
    pub mult: i32,
    pub ttl: i32,
    pub flags: i32,
    pub open_coef: f32,
    pub exec_prio: f32,
    pub setup: i32,
    pub qty: i32,
    pub active: bool,
    pub closed: bool,
    pub comment: Option<String>,
}