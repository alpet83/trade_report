// Modified: 2025-06-19 15:55:00 EEST
// xaiArtifact: artifact_id="3a1b2c3d-4e5f-6a7b-8c9d-0e1f2a3b4c5d", artifact_version_id="4a5b6c7d-8e9f-0a1b-2c3d-4e5f6a7b8c9d"

use async_trait::async_trait;
use sqlx::MySqlPool;
use chrono::{DateTime, Utc};

use crate::{
    entities::{
        trade::{Trade, Order, OrdersBatch, TradeSignal},
        account_data::{FundsHistoryRow, DepositHistoryRow},
        ticker::TickerInfo,
        position::PositionHistory,
        report::ReportConfig,
    },
    db::load_equity_data::LoadEquityData,
};

pub struct MySqlDataSource {
    pub pool: MySqlPool,
}

impl MySqlDataSource {
    pub async fn new(url: &str) -> Result<Self, String> {
        let pool = MySqlPool::connect(url)
            .await
            .map_err(|e| format!("Failed to connect to MySQL: {}", e))?;
        Ok(MySqlDataSource { pool })
    }
}

#[async_trait]
pub trait TradeDataSource: LoadEquityData {
    async fn get_trades(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        exchange: &str,
        pair_id: Option<i32>,
        account_id: i32,
    ) -> Result<Vec<Trade>, String>;

    async fn get_funds_history(
        &self,
        exchange: &str,
        account_id: i32,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<FundsHistoryRow>, String>;

    async fn get_orders(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        exchange: &str,
        pair_id: Option<i32>,
        account_id: i32,
        status: Option<&str>,
    ) -> Result<Vec<Order>, String>;

    async fn get_candle_price(
        &self,
        ts: DateTime<Utc>,
        exchange: &str,
        pair: &str,
        use_clickhouse: bool,
    ) -> Result<f64, String>;

    async fn get_ticker_info(
        &self,
        exchange: &str,
        pair_id: i32,
    ) -> Result<TickerInfo, String>;

    async fn get_deposit_history(
        &self,
        exchange: &str,
        account_id: i32,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<DepositHistoryRow>, String>;

    async fn get_position_history(
        &self,
        exchange: &str,
        pair_id: i32,
        account_id: i32,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<PositionHistory>, String>;

    async fn get_trade_signals(
        &self,
        exchange: &str,
        account_id: i32,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<TradeSignal>, String>;

    async fn get_report_configs(
        &self,
        exchange: &str,
    ) -> Result<Vec<ReportConfig>, String>;
}

