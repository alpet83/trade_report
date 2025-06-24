// /src/entities/cache.rs
// Modified: 2025-06-24 10:41:00 EEST

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use std::sync::Arc;

use crate::{
    entities::exchange::Exchange,
    entities::trade::Trade,
    entities::account::TradingAccount,
    entities::task::TaskBase,
};

// Caches VWAP prices for an exchange and pair
#[derive(Debug)]
pub struct PriceCache {
    pub data: DashMap<i32, f32>,
    pub exchange: Arc<Exchange>,
    pub pair_id: Option<i32>,
}

// Task for loading price cache data in the background
#[derive(Debug, Clone)]
pub struct LoadPriceCacheTask {
    pub base: TaskBase,
    pub cache: Arc<PriceCache>,
    pub start_ts: DateTime<Utc>,
    pub end_ts: DateTime<Utc>,
}

// Caches trades for an account and pair
#[derive(Debug)]
pub struct TradesCache {
    pub data: DashMap<i32, Vec<Trade>>,
    pub account: Arc<TradingAccount>,
    pub pair_id: i32,
}