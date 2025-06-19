// Modified: 2025-06-19 12:45:00 EEST
// xaiArtifact: artifact_id="16c678c2-0a49-4e86-97b1-228913ed9431", artifact_version_id="9c0d1e2f-3a4b-5c6d-7e8f-9a0b1c2d3e4f"

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{MySqlPool, Row};
use tracing::{info, error, debug};

use crate::{
    entities::{
        trade::{Trade, Order, OrdersBatch, TradeSignal},
        account_data::{DepositHistoryRow, FundsHistoryRow},
        ticker::TickerInfo,
        position::PositionHistory,
        report::ReportConfig,
    },
    db::{
        mysql::{MySqlDataSource, TradeDataSource},
        error::handle_sql_error,
        load_equity_data::LoadEquityData,
    },
};

#[async_trait]
impl TradeDataSource for MySqlDataSource {
    async fn get_trades(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        exchange: &str,
        pair_id: Option<i32>,
        account_id: i32,
    ) -> Result<Vec<Trade>, String> {
        let table = format!("{}__trades", exchange.to_lowercase());
        let mut query = String::from(format!(
            "SELECT * FROM {} WHERE ts >= ? AND ts <= ? AND account_id = ?",
            table
        ));

        if pair_id.is_some() {
            query.push_str(" AND pair_id = ?");
        }

        debug!("Executing query: {} with account_id={}, exchange={}, start_ts={}, end_ts={}", query, account_id, exchange, start, end);
        let mut query_builder = sqlx::query_as::<_, Trade>(&query)
            .bind(start)
            .bind(end)
            .bind(account_id);

        if let Some(pid) = pair_id {
            query_builder = query_builder.bind(pid);
        }

        match query_builder.fetch_all(&self.pool).await {
            Ok(trades) => {
                info!("Fetched {} trades from {} for account_id={}", trades.len(), table, account_id);
                Ok(trades)
            }
            Err(e) => {
                Err(handle_sql_error(&query, e))
            }
        }
    }

    async fn get_funds_history(
        &self,
        exchange: &str,
        account_id: i32,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<FundsHistoryRow>, String> {
        let table = format!("{}__funds_history", exchange.to_lowercase());
        let query = format!(
            "SELECT ts, value, value_btc, position_coef FROM {} WHERE ts >= ? AND ts <= ? AND account_id = ? ORDER BY ts",
            table
        );

        debug!("Executing query: {} with account_id={}, exchange={}, start_ts={}, end_ts={}", query, account_id, exchange, start, end);
        match sqlx::query_as::<_, FundsHistoryRow>(&query)
            .bind(start)
            .bind(end)
            .bind(account_id)
            .fetch_all(&self.pool)
            .await
        {
            Ok(history) => {
                info!("Fetched {} funds history records from {} for account_id={}", history.len(), table, account_id);
                Ok(history)
            }
            Err(e) => {
                Err(handle_sql_error(&query, e))
            }
        }
    }

    async fn get_orders(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        exchange: &str,
        pair_id: Option<i32>,
        account_id: i32,
        status: Option<&str>,
    ) -> Result<Vec<Order>, String> {
        let table = format!("{}__orders", exchange.to_lowercase());
        let mut query = String::from(format!(
            "SELECT * FROM {} WHERE ts >= ? AND ts <= ? AND account_id = ?",
            table
        ));

        if pair_id.is_some() {
            query.push_str(" AND pair_id = ?");
        }
        if status.is_some() {
            query.push_str(" AND status = ?");
        }

        debug!("Executing query: {} with account_id={}, exchange={}, start_ts={}, end_ts={}", query, account_id, exchange, start, end);
        let mut query_builder = sqlx::query_as::<_, Order>(&query)
            .bind(start)
            .bind(end)
            .bind(account_id);

        if let Some(pid) = pair_id {
            query_builder = query_builder.bind(pid);
        }
        if let Some(st) = status {
            query_builder = query_builder.bind(st);
        }

        match query_builder.fetch_all(&self.pool).await {
            Ok(orders) => {
                info!("Fetched {} orders from {} for account_id={}", orders.len(), table, account_id);
                Ok(orders)
            }
            Err(e) => {
                Err(handle_sql_error(&query, e))
            }
        }
    }

    async fn get_deposit_history(
        &self,
        exchange: &str,
        account_id: i32,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<DepositHistoryRow>, String> {
        let table = format!("{}__deposit_history", exchange.to_lowercase());
        let query = format!(
            "SELECT ts, withdrawal, value_usd, value_btc FROM {} WHERE ts >= ? AND ts <= ? AND account_id = ? ORDER BY ts",
            table
        );

        debug!("Executing query: {} with account_id={}, exchange={}, start_ts={}, end_ts={}", query, account_id, exchange, start, end);
        match sqlx::query_as::<_, DepositHistoryRow>(&query)
            .bind(start)
            .bind(end)
            .bind(account_id)
            .fetch_all(&self.pool)
            .await
        {
            Ok(history) => {
                info!("Fetched {} deposit history records from {} for account_id={}", history.len(), table, account_id);
                Ok(history)
            }
            Err(e) => {
                Err(handle_sql_error(&query, e))
            }
        }
    }

    async fn get_candle_price(
        &self,
        _ts: DateTime<Utc>,
        _exchange: &str,
        _pair: &str,
        _use_clickhouse: bool,
    ) -> Result<f64, String> {
        Err("Not implemented".to_string())
    }

    async fn get_ticker_info(&self, _exchange: &str, _pair_id: i32) -> Result<TickerInfo, String> {
        Err("Not implemented".to_string())
    }

    async fn get_position_history(
        &self,
        _exchange: &str,
        _pair_id: i32,
        _account_id: i32,
        _start: DateTime<Utc>,
        _end: DateTime<Utc>,
    ) -> Result<Vec<PositionHistory>, String> {
        Err("Not implemented".to_string())
    }

    async fn get_trade_signals(
        &self,
        _exchange: &str,
        _account_id: i32,
        _start: DateTime<Utc>,
        _end: DateTime<Utc>,
    ) -> Result<Vec<TradeSignal>, String> {
        Err("Not implemented".to_string())
    }

    async fn get_report_configs(&self, _exchange: &str) -> Result<Vec<ReportConfig>, String> {
        Err("Not implemented".to_string())
    }
}