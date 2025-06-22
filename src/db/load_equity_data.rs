// /src/db/load_equity_data.rs
// Modified: 2025-06-22 08:53:00 EEST

use chrono::{DateTime, Utc, Timelike};
use async_trait::async_trait;

use crate::{
    entities::account::TradingAccount,
    entities::account_data::{FundsHistoryRow, DepositHistoryRow},    
    entities::public_data::PublicDataSource,
    entities::trade_data::TradeDataSource,
    db::{mysql::{MySqlDataSource}},
    common::consts::BTC_PAIR_ID,
};

// Loads equity data for an account, adjusting for deposits and withdrawals over time
#[async_trait]
pub trait LoadEquityData {
    async fn load_equity_data(
        &self,        
        account: &TradingAccount,
        start_ts: DateTime<Utc>,
        end_ts: DateTime<Utc>,
        value_column: &str,
    ) -> Result<Vec<(DateTime<Utc>, f32)>, String>;
}

#[async_trait]
impl LoadEquityData for MySqlDataSource {
    // Loads equity data by fetching funds history, adjusting for deposits/withdrawals, and using PriceCache for BTC prices
    async fn load_equity_data(
        &self,        
        account: &TradingAccount,
        start_ts: DateTime<Utc>,
        end_ts: DateTime<Utc>,
        value_column: &str,
    ) -> Result<Vec<(DateTime<Utc>, f32)>, String> {
        let account_id = account.account_id;            
        let exchange = &account.exchange.name;

        let mut funds = self.get_funds_history(account, start_ts, end_ts)
            .await
            .map_err(|e| format!("Failed to fetch funds history: {}", e))?;
        funds.sort_by(|a, b| a.ts.cmp(&b.ts)); // Ensure chronological order

        let mut deposits = self.get_deposit_history(account, end_ts)
            .await
            .map_err(|e| format!("Failed to fetch deposit history: {}", e))?;
        deposits.sort_by(|a, b| a.ts.cmp(&b.ts)); // Ensure chronological order

        let mut equity_points = Vec::new();
        let mut accum_usd = 0.0;
        let mut accum_btc = 0.0;
        let mut fund_idx = 0;

        let cache = account.exchange.get_price_cache(Some(BTC_PAIR_ID)).await;

        // Add sentinel deposit to handle remaining funds
        deposits.push(DepositHistoryRow {
            ts: end_ts + chrono::Duration::seconds(1),
            withdrawal: false,
            value_usd: 0.0,
            value_btc: 0.0,
        });

        for dep in deposits {
            let dep_ts = dep.ts;

            // Process all funds points before or at the deposit time
            while fund_idx < funds.len() && funds[fund_idx].ts <= dep_ts {
                let fund = &funds[fund_idx];
                let btc_price = cache.get_vwap(self as &dyn PublicDataSource, fund.ts)
                    .await
                    .map_err(|e| format!("Failed to fetch BTC price: {}", e))?;

                let usd_coef = if btc_price > 0.0 { 1.0 / btc_price } else { 0.0 };
                let btc_coef = btc_price;

                let value = match value_column {
                    "value_btc" => fund.value_btc - accum_btc - accum_usd * usd_coef,
                    _ => fund.value - accum_usd - accum_btc * btc_coef,
                };

                let ts = fund.ts
                    .with_second(0)
                    .expect("Invalid datetime")
                    .with_nanosecond(0)
                    .expect("Invalid datetime");

                equity_points.push((ts, value));
                fund_idx += 1;
            }

            // Update accumulated sums for the current deposit/withdrawal
            let sign = if dep.withdrawal { -1.0 } else { 1.0 };
            accum_usd += dep.value_usd * sign;
            accum_btc += dep.value_btc * sign;
        }

        Ok(equity_points)
    }
}