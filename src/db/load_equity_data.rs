// Modified: 2025-06-19 12:45:00 EEST
// xaiArtifact: artifact_id="4f63b968-1f14-4317-8903-a78b7c3b2faa", artifact_version_id="6a7b8c9d-0e1f-2a3b-4c5d-6e7f8a9b0c1d"

use chrono::{DateTime, Utc, Timelike};
use async_trait::async_trait;

use crate::{
    entities::account_data::{FundsHistoryRow, DepositHistoryRow},
    db::{mysql::{MySqlDataSource, TradeDataSource}, error::handle_sql_error},
};

#[async_trait]
pub trait LoadEquityData {
    async fn load_equity_data(
        &self,
        exchange: &str,
        account_id: i32,
        start_ts: DateTime<Utc>,
        end_ts: DateTime<Utc>,
        value_column: &str,
    ) -> Result<Vec<(DateTime<Utc>, f32)>, String>;
}

#[async_trait]
impl LoadEquityData for MySqlDataSource {
    async fn load_equity_data(
        &self,
        exchange: &str,
        account_id: i32,
        start_ts: DateTime<Utc>,
        end_ts: DateTime<Utc>,
        value_column: &str,
    ) -> Result<Vec<(DateTime<Utc>, f32)>, String> {
        let table_funds = format!("{}__funds_history", exchange.to_lowercase());
        let query_funds = format!(
            "SELECT ts, value, value_btc FROM {} WHERE ts >= ? AND ts <= ? AND account_id = ? ORDER BY ts",
            table_funds
        );

        let funds = sqlx::query_as::<_, FundsHistoryRow>(&query_funds)
            .bind(start_ts)
            .bind(end_ts)
            .bind(account_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| handle_sql_error(&query_funds, e))?;

        let table_dep = format!("{}__deposit_history", exchange.to_lowercase());
        let query_dep = format!(
            "SELECT ts, withdrawal, value_usd, value_btc FROM {} WHERE ts >= ? AND ts <= ? AND account_id = ? ORDER BY ts",
            table_dep
        );

        let deposits = sqlx::query_as::<_, DepositHistoryRow>(&query_dep)
            .bind(start_ts)
            .bind(end_ts)
            .bind(account_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| handle_sql_error(&query_dep, e))?;

        let mut equity_points = Vec::new();
        let mut accum_usd = 0.0;
        let mut accum_btc = 0.0;
        let btc_price = 80000.0; // Placeholder, as in draw_chart.php

        for fund in funds {
            // Accumulate deposits/withdrawals up to fund's timestamp
            for dep in deposits.iter().filter(|d| d.ts <= fund.ts) {
                let sign = if dep.withdrawal { -1.0 } else { 1.0 };
                accum_usd += dep.value_usd * sign;
                accum_btc += dep.value_btc * sign;
            }

            let value = match value_column {
                "value_btc" => fund.value_btc - accum_btc - accum_usd / btc_price,
                _ => fund.value - accum_usd - accum_btc * btc_price,
            };

            // Round timestamp to minute
            let ts = fund.ts
                .with_second(0)
                .expect("Invalid datetime")
                .with_nanosecond(0)
                .expect("Invalid datetime");

            equity_points.push((ts, value));
        }

        Ok(equity_points)
    }
}