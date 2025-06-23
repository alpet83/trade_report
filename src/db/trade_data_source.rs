// /src/db/trade_data_source.rs
// Modified: 2025-06-22 14:15:00 EEST

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{MySql, Row};
use tracing::{info, error, debug};

use crate::{
    entities::{
        trade::{Trade, Order, TradeSignal},
        account_data::{FundsHistoryRow, DepositHistoryRow},
        ticker::TickerInfo,
        position::PositionHistory,
        report::ReportConfig,
        account::TradingAccount,
        trade_data::TradeDataSource,
    },
    db::{
        mysql::{MySqlDataSource, trading_table_name},
        error::handle_sql_error,
        load_equity_data::LoadEquityData,
    },
    build_query,
};

#[async_trait]
impl TradeDataSource for MySqlDataSource {
    // Fetches trades for an account
    async fn get_trades(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        account: &TradingAccount,
        pair_id: Option<u32>,
    ) -> Result<Vec<Trade>, String> {
        let table = trading_table_name(&account.exchange.name, "trades");
        let query = build_query!(
            table,
            "SELECT * FROM {} WHERE ts >= ? AND ts <= ? AND account_id = ?",
            pair_id.is_some().then(|| " AND pair_id = ?").unwrap_or("")
        );

        debug!("Executing query: {} with account_id={}, exchange={}, start_ts={}, end_ts={}", query, account.account_id, account.exchange.name, start, end);
        let mut query_builder = sqlx::query_as::<_, Trade>(&query)
            .bind(start)
            .bind(end)
            .bind(account.account_id as i32);

        if let Some(pid) = pair_id {
            query_builder = query_builder.bind(pid as i32);
        }

        match query_builder.fetch_all(&self.pool).await {
            Ok(trades) => {
                info!("Fetched {} trades from {} for account_id={}", trades.len(), table, account.account_id);
                Ok(trades)
            }
            Err(e) => {
                Err(handle_sql_error(&query, e))
            }
        }
    }

    // Fetches funds history with USD and BTC balances
    async fn get_funds_history(
        &self,
        account: &TradingAccount,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<FundsHistoryRow>, String> {
        let table = trading_table_name(&account.exchange.name, "funds_history");
        let query = build_query!(
            table,
            "SELECT ts, value, value_btc, position_coef FROM {} WHERE ts >= ? AND ts <= ? AND account_id = ? ORDER BY ts",
        );

        debug!("Executing query: {} with account_id={}, exchange={}, start_ts={}, end_ts={}", query, account.account_id, account.exchange.name, start, end);
        match sqlx::query_as::<_, FundsHistoryRow>(&query)
            .bind(start)
            .bind(end)
            .bind(account.account_id as i32)
            .fetch_all(&self.pool)
            .await
        {
            Ok(history) => {
                info!("Fetched {} funds history records from {} for account_id={}", history.len(), table, account.account_id);
                Ok(history)
            }
            Err(e) => {
                Err(handle_sql_error(&query, e))
            }
        }
    }

    // Fetches aggregated funds history with USD and BTC balances for large periods, grouped by 4-hour intervals
    async fn get_funds_history_aggregated(
        &self,
        account: &TradingAccount,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<FundsHistoryRow>, String> {
        let table = trading_table_name(&account.exchange.name, "funds_history");
        let query = format!(
            "SELECT 
                FROM_UNIXTIME(FLOOR(UNIX_TIMESTAMP(ts) / (4 * 3600)) * (4 * 3600)) AS ts,
                AVG(value) AS value,
                AVG(value_btc) AS value_btc,
                AVG(position_coef) AS position_coef
            FROM {} 
            WHERE ts >= ? AND ts <= ? AND account_id = ? 
            GROUP BY FLOOR(UNIX_TIMESTAMP(ts) / (4 * 3600))
            ORDER BY ts",
            table
        );

        debug!("Executing aggregated query: {} with account_id={}, exchange={}, start_ts={}, end_ts={}", query, account.account_id, account.exchange.name, start, end);
        match sqlx::query_as::<_, FundsHistoryRow>(&query)
            .bind(start)
            .bind(end)
            .bind(account.account_id as i32)
            .fetch_all(&self.pool)
            .await
        {
            Ok(history) => {
                info!("Fetched {} aggregated funds history records from {} for account_id={}", history.len(), table, account.account_id);
                Ok(history)
            }
            Err(e) => {
                Err(handle_sql_error(&query, e))
            }
        }
    }

    // Fetches orders for an account
    async fn get_orders(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        account: &TradingAccount,
        pair_id: Option<u32>,
        status: Option<&str>,
    ) -> Result<Vec<Order>, String> {
        let table = trading_table_name(&account.exchange.name, "orders");
        let query = build_query!(
            table,
            "SELECT * FROM {} WHERE ts >= ? AND ts <= ? AND account_id = ?",
            pair_id.is_some().then(|| " AND pair_id = ?").unwrap_or(""),
            status.is_some().then(|| " AND status = ?").unwrap_or("")
        );

        debug!("Executing query: {} with account_id={}, exchange={}, start_ts={}, end_ts={}", query, account.account_id, account.exchange.name, start, end);
        let mut query_builder = sqlx::query_as::<_, Order>(&query)
            .bind(start)
            .bind(end)
            .bind(account.account_id as i32);

        if let Some(pid) = pair_id {
            query_builder = query_builder.bind(pid as i32);
        }
        if let Some(st) = status {
            query_builder = query_builder.bind(st);
        }

        match query_builder.fetch_all(&self.pool).await {
            Ok(orders) => {
                info!("Fetched {} orders from {} for account_id={}", orders.len(), table, account.account_id);
                Ok(orders)
            }
            Err(e) => {
                Err(handle_sql_error(&query, e))
            }
        }
    }

    // Fetches deposit and withdrawal history
    async fn get_deposit_history(
        &self,
        account: &TradingAccount,        
        end: DateTime<Utc>,
    ) -> Result<Vec<DepositHistoryRow>, String> {
        let table = trading_table_name(&account.exchange.name, "deposit_history");
        let query = build_query!(
            table,
            "SELECT ts, withdrawal, value_usd, value_btc FROM {} WHERE ts <= ? AND account_id = ? ORDER BY ts",
        );

        debug!("Executing query: {} with account_id={}, exchange={}, end_ts={}", query, account.account_id, account.exchange.name, end);
        match sqlx::query_as::<_, DepositHistoryRow>(&query)            
            .bind(end)
            .bind(account.account_id as i32)
            .fetch_all(&self.pool)
            .await
        {
            Ok(history) => {
                info!("Fetched {} deposit history records from {} for account_id={}", history.len(), table, account.account_id);
                Ok(history)
            }
            Err(e) => {
                Err(handle_sql_error(&query, e))
            }
        }
    }

    // Fetches position history (not implemented)
    async fn get_position_history(
        &self,
        account: &TradingAccount,
        pair_id: u32,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<PositionHistory>, String> {
        Err("Not implemented".to_string())
    }

    // Fetches trade signals (not implemented)
    async fn get_trade_signals(
        &self,
        account: &TradingAccount,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<TradeSignal>, String> {
        Err("Not implemented".to_string())
    }

    // Fetches report configurations (not implemented)
    async fn get_report_configs(&self, exchange: &str) -> Result<Vec<ReportConfig>, String> {
        Err("Not implemented".to_string())
    }
}