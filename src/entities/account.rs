// /src/entities/account.rs
// Modified: 2025-06-24 08:24:00 EEST

use std::sync::Arc;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use once_cell::sync::OnceCell;
use tokio::sync::RwLock;
use tracing::{info, error, debug};
use dashmap::DashMap;

use crate::{
    entities::{
        exchange::Exchange,
        account_data::{DepositHistoryRow, FundsHistoryRow},
        trade::{Trade, Order},
        cache::TradesCache,
    },
    db::mysql::MySqlDataSource,
    config::BotConfigMap,
    logs::app_error::AppError
};
use sqlx::MySqlPool;

pub async fn resolve_account(
    exchange: Option<String>,
    account_id: Option<String>,
    applicant: Option<String>,
) -> Result<TradingAccount, AppError> {
    let manager = get_account_manager();
    match (exchange, account_id, applicant) {
        (Some(exchange), Some(account_id), None) => {
            let account_id = account_id.parse::<u32>()
                .map_err(|e| AppError::Internal(format!("Invalid account_id: {}", e)))?;
            let guard = manager.read().await;
            guard.get_account(&account_id)
                .filter(|acc| acc.exchange.name.to_lowercase() == exchange.to_lowercase())
                .ok_or_else(|| AppError::Internal(format!("No account found for account_id={} on exchange={}", account_id, exchange)))
                .map(|acc| acc.clone())
        }
        (None, None, Some(applicant)) => {
            let guard = manager.read().await;
            guard.find_account(&applicant)
                .ok_or_else(|| AppError::Internal(format!("No account found for applicant: {}", applicant)))
                .map(|acc| acc.clone())
        }
        _ => Err(AppError::Internal("Must provide either (exchange, account_id) or applicant".to_string())),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingAccount {
    pub account_id: u32,
    #[serde(rename = "applicant_name")]
    pub applicant: String,
    pub exchange: Arc<Exchange>,
    #[serde(rename = "monitor_enabled", serialize_with = "serialize_bool_as_str")]
    pub monitor_enabled: bool,
    #[serde(skip)]
    pub deposit_history: HashMap<String, Vec<DepositHistoryRow>>,
    #[serde(skip)]
    pub funds_history: HashMap<String, Vec<FundsHistoryRow>>,
    #[serde(skip)]
    pub trades_caches: Arc<DashMap<i32, Arc<TradesCache>>>,
    #[serde(skip)]
    pub orders: HashMap<String, Vec<Order>>,
}

impl TradingAccount {
    pub fn new(account_id: u32, applicant: String, exchange: Arc<Exchange>, monitor_enabled: bool) -> Self {
        Self {
            account_id,
            applicant,
            exchange,
            monitor_enabled,
            deposit_history: HashMap::new(),
            funds_history: HashMap::new(),
            trades_caches: Arc::new(DashMap::new()),
            orders: HashMap::new(),
        }
    }

    // Retrieves or creates a TradesCache for a given pair ID
    pub async fn get_trades_cache(&self, pair_id: i32) -> Arc<TradesCache> {
        self.trades_caches.entry(pair_id).or_insert_with(|| {
            Arc::new(TradesCache::new(Arc::new(self.clone()), pair_id))
        }).clone()
    }
}

#[derive(Debug)]
pub struct TradingAccountManager {
    accounts: HashMap<u32, TradingAccount>,
}

impl TradingAccountManager {
    pub fn new() -> Self {
        Self {
            accounts: HashMap::new(),
        }
    }

    pub async fn initialize(&mut self) -> Result<(), String> {
        info!("Starting initialization of TradingAccountManager");
        debug!("Loading BotConfigMap");
        let db = MySqlDataSource::db_conn();
        let config_map = BotConfigMap::load(&db.pool)
            .await
            .map_err(|e| {
                error!("Failed to load bot config map: {}", e);
                format!("Failed to load bot config map: {}", e)
            })?;

        debug!("Processing {} bot configs", config_map.configs.len());
        for (applicant, bot_config) in config_map.configs {
            debug!("Creating account for applicant: {}", applicant);
            let account_id = bot_config.account_id;
            let exchange = Arc::new(Exchange::new(bot_config.exchange).await);
            let trading_account = TradingAccount::new(
                account_id,
                applicant,
                exchange,
                bot_config.monitor_enabled,
            );
            self.accounts.insert(trading_account.account_id, trading_account);
        }

        info!("Initialized {} trading accounts", self.accounts.len());
        Ok(())
    }

    pub fn add_account(&mut self, account: TradingAccount) {
        self.accounts.insert(account.account_id, account);
    }

    pub fn get_account(&self, account_id: &u32) -> Option<&TradingAccount> {
        self.accounts.get(account_id)
    }

    pub fn find_account(&self, applicant: &str) -> Option<&TradingAccount> {
        self.accounts.values().find(|account| account.applicant == applicant)
    }

    pub fn list_accounts(&self) -> Vec<&TradingAccount> {
        self.accounts.values().collect()
    }
}

static ACCOUNT_MANAGER: OnceCell<Arc<RwLock<TradingAccountManager>>> = OnceCell::new();

pub fn create_account_manager() -> Arc<RwLock<TradingAccountManager>> {
    let manager = Arc::new(RwLock::new(TradingAccountManager::new()));
    ACCOUNT_MANAGER
        .set(manager.clone())
        .expect("AccountManager already initialized");
    info!("Created TradingAccountManager (singleton instance)");
    manager
}

pub fn get_account_manager() -> Arc<RwLock<TradingAccountManager>> {
    ACCOUNT_MANAGER
        .get()
        .expect("AccountManager not initialized")
        .clone()
}

fn serialize_bool_as_str<S>(value: &bool, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(if *value { "1" } else { "0" })
}