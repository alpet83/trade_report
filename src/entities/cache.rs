use chrono::{DateTime, Utc, Duration};
use async_trait::async_trait;
use dashmap::DashMap;

use crate::entities::trade::Trade;
use crate::db::mysql::TradeDataSource;

#[async_trait]
pub trait TradingDataCache: Send + Sync {
    fn exchange(&self) -> &str;
    fn start(&self) -> DateTime<Utc>;
    fn end(&self) -> DateTime<Utc>;
    fn interval(&self) -> Duration;
    fn pair_id(&self) -> Option<i32>;
    fn len(&self) -> usize;
    fn clear(&mut self);
    async fn load(&mut self, db: &dyn TradeDataSource) -> Result<(), String>;
}

pub struct TradesCache {
    pub cache: DashMap<i32, Trade>,
    pub exchange: String,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub interval: Duration,
    pub pair_id: Option<i32>,
}