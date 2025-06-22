
/* Здесь определен только интерфейс TradeDataSource, имплементацию смотри в /src/db/trade_data_source.rs */
use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::{
    db::load_equity_data::LoadEquityData,
    entities::{account::TradingAccount, account_data::{FundsHistoryRow, DepositHistoryRow}, 
    trade::Trade, trade::Order, position::PositionHistory, trade::TradeSignal, report::ReportConfig},
};

// Defines interface for accessing trade-related data
#[async_trait]
pub trait TradeDataSource: Send + Sync + LoadEquityData {
    async fn get_trades(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        account: &TradingAccount, // Изменено
        pair_id: Option<u32>, // Изменено с i32 на u32
    ) -> Result<Vec<Trade>, String>;

    async fn get_funds_history(
        &self,
        account: &TradingAccount, // Изменено
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<FundsHistoryRow>, String>;

    async fn get_orders(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        account: &TradingAccount, 
        pair_id: Option<u32>, 
        status: Option<&str>,
    ) -> Result<Vec<Order>, String>;
    
    async fn get_deposit_history(
        &self,
        account: &TradingAccount,         
        end: DateTime<Utc>,
    ) -> Result<Vec<DepositHistoryRow>, String>;

    async fn get_position_history(
        &self,
        account: &TradingAccount, 
        pair_id: u32, 
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<PositionHistory>, String>;

    async fn get_trade_signals(
        &self,
        account: &TradingAccount, 
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<TradeSignal>, String>;

    async fn get_report_configs(
        &self,
        exchange: &str, 
    ) -> Result<Vec<ReportConfig>, String>;
}