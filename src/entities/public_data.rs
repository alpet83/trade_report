use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::MySqlPool;
use tracing::{info, debug, error};

use crate::{
    db::{mysql::{MySqlDataSource, public_table_name}, error::handle_sql_error},
    common::time,
    logs::app_error::AppError,
};

// Структура для свечных данных
#[derive(sqlx::FromRow, Debug, Clone)]
pub struct Candle {
    pub ts: DateTime<Utc>,
    pub open: f32,
    pub high: f32,
    pub low: f32,
    pub close: f32,
    pub volume: f32,
}

// Структура для записи ticker_map
#[derive(sqlx::FromRow)]
pub struct TickerMapRow {
    pub id: i32,
    pub ticker: String,
    pub symbol: String,
    pub pair_id: Option<i32>,
}

#[async_trait]
pub trait PublicDataSource: Send + Sync {
    async fn load_candles(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        exchange: &str,
        pair_id: Option<i32>,
    ) -> Result<Vec<Candle>, AppError>;

    async fn get_ticker(
        &self,
        exchange: &str,
        pair_id: i32,
    ) -> Result<String, AppError>;
}

#[async_trait]
impl PublicDataSource for MySqlDataSource {
    async fn load_candles(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        exchange: &str,
        pair_id: Option<i32>,
    ) -> Result<Vec<Candle>, AppError> {
        let ticker = match pair_id {
            Some(pid) => self.get_ticker(exchange, pid).await?,
            None => "BTCUSD".to_string(), // Значение по умолчанию
        };

        let table = public_table_name(exchange, &format!("candles__{}", ticker));
        let query = format!(
            "SELECT ts, open, high, low, close, volume FROM {} WHERE ts >= ? AND ts <= ? ORDER BY ts",
            table
        );

        debug!("Executing candles query: {} with exchange={}, start_ts={}, end_ts={}", query, exchange, start, end);
        let candles = sqlx::query_as::<_, Candle>(&query)
            .bind(start)
            .bind(end)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| AppError::Internal(handle_sql_error(&query, e)))?;

        info!("Fetched {} candles from {} for pair_id={:?}", candles.len(), table, pair_id);
        Ok(candles)
    }

    async fn get_ticker(
        &self,
        exchange: &str,
        pair_id: i32,
    ) -> Result<String, AppError> {
        let table = public_table_name(exchange, "ticker_map");
        let query = format!(
            "SELECT * FROM {} WHERE pair_id = ? -- detect ticker name by pair_id",
            table
        );

        debug!("Executing ticker query: {} with exchange={}, pair_id={}", query, exchange, pair_id);
        let row = sqlx::query_as::<_, TickerMapRow>(&query)
            .bind(pair_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| AppError::Internal(handle_sql_error(&query, e)))?;

        Ok(row.ticker)
    }
}