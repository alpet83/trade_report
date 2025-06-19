use std::sync::Arc;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use once_cell::sync::OnceCell;
use tokio::sync::RwLock;
use tracing::{info, error, debug};

use crate::{entities::{exchange::Exchange, account_data::{DepositHistoryRow, FundsHistoryRow}, trade::{Trade, Order}}, config::BotConfigMap};
use sqlx::MySqlPool;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingAccount {
    pub account_id: String,
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
    pub trades: HashMap<String, Vec<Trade>>,
    #[serde(skip)]
    pub orders: HashMap<String, Vec<Order>>,
}

impl TradingAccount {
    pub fn new(account_id: String, applicant: String, exchange: Arc<Exchange>, monitor_enabled: bool) -> Self {
        Self {
            account_id,
            applicant,
            exchange,
            monitor_enabled,
            deposit_history: HashMap::new(),
            funds_history: HashMap::new(),
            trades: HashMap::new(),
            orders: HashMap::new(),
        }
    }
}

#[derive(Debug)]
pub struct TradingAccountManager {
    accounts: HashMap<String, TradingAccount>,
}

impl TradingAccountManager {
    pub fn new() -> Self {
        Self {
            accounts: HashMap::new(),
        }
    }

    pub async fn initialize(&mut self, pool: &MySqlPool) -> Result<(), String> {
        info!("Starting initialization of TradingAccountManager");

        debug!("Loading BotConfigMap");
        let config_map = BotConfigMap::load(pool)
            .await
            .map_err(|e| {
                error!("Failed to load bot config map: {}", e);
                format!("Failed to load bot config map: {}", e)
            })?;

        debug!("Processing {} bot configs", config_map.configs.len());
        for (applicant, bot_config) in config_map.configs {
            debug!("Creating account for applicant: {}", applicant);
            let trading_account = TradingAccount::new(
                bot_config.account_id.to_string(),
                applicant,
                Arc::new(Exchange::new(bot_config.exchange)),
                bot_config.monitor_enabled,
            );
            self.accounts.insert(trading_account.account_id.clone(), trading_account);
        }

        info!("Initialized {} trading accounts", self.accounts.len());
        Ok(())
    }

    pub fn add_account(&mut self, account: TradingAccount) {
        self.accounts.insert(account.account_id.clone(), account);
    }

    pub fn get_account(&self, account_id: &str) -> Option<&TradingAccount> {
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

// Custom serializer for monitor_enabled to output "1" or "0"
fn serialize_bool_as_str<S>(value: &bool, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(if *value { "1" } else { "0" })
}