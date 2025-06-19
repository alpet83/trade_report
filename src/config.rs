use std::path::PathBuf;
use sqlx::{MySqlPool, Row};
use tracing::{info, error, debug};
use serde::Deserialize;
use std::fs;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub mysql: MysqlConfig,
    pub server: ServerConfig,
    pub logging: LoggingConfig,
}

#[derive(Debug, Deserialize)]
pub struct MysqlConfig {
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub api_port: u16,
}

#[derive(Debug, Deserialize)]
pub struct LoggingConfig {
    pub log_dir: String,
}

impl Config {
    pub fn load() -> Result<Self, String> {
        let config_path = std::env::var("CONFIG_PATH").unwrap_or("config.toml".to_string());
        debug!("Reading config from {}", config_path);
        let config_str = fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read {}: {}", config_path, e))?;
        let config: Config = toml::from_str(&config_str)
            .map_err(|e| format!("Failed to parse {}: {}", config_path, e))?;
        info!("Loaded config from {}", config_path);
        Ok(config)
    }

    pub fn log_dir(&self) -> PathBuf {
        PathBuf::from(&self.logging.log_dir)
    }
}

#[derive(Debug)]
pub struct BotConfig {
    pub account_id: i32,
    pub exchange: String,
    pub monitor_enabled: bool,
}

#[derive(Debug)]
pub struct BotConfigMap {
    pub configs: HashMap<String, BotConfig>,
}

impl BotConfigMap {
    pub async fn load(pool: &MySqlPool) -> Result<Self, String> {
        info!("Loading bot config map from config__table_map");

        debug!("Executing query: SELECT applicant, table_name FROM config__table_map WHERE table_key = 'config'");
        let mappings: Vec<(String, String)> = sqlx::query_as("SELECT applicant, table_name FROM config__table_map WHERE table_key = 'config'")
            .fetch_all(pool)
            .await
            .map_err(|e| {
                error!("Failed to fetch config__table_map: {}", e);
                format!("Failed to fetch config__table_map: {}", e)
            })?;

        let mut configs = HashMap::new();

        for (applicant, table_name) in mappings {
            debug!("Processing table: {} for applicant: {}", table_name, applicant);
            let query = format!("SELECT account_id, param, value FROM {}", table_name);
            let rows = sqlx::query(&query)
                .fetch_all(pool)
                .await
                .map_err(|e| {
                    error!("Failed to fetch {}: {}", table_name, e);
                    format!("Failed to fetch {}: {}", table_name, e)
                })?;

            let mut config_map = HashMap::new();
            let mut account_id = None;

            for row in rows {
                let param: String = row.get(1);
                let value: String = row.get(2);
                let current_account_id: i32 = row.get(0);

                if account_id.is_none() {
                    account_id = Some(current_account_id);
                } else if account_id != Some(current_account_id) {
                    return Err(format!("Inconsistent account_id in {}", table_name));
                }

                config_map.insert(param, value);
            }

            let account_id = account_id.ok_or_else(|| {
                error!("No account_id found in {}", table_name);
                format!("No account_id found in {}", table_name)
            })?;
            let exchange = config_map.get("exchange")
                .ok_or_else(|| {
                    error!("Missing exchange in {}", table_name);
                    format!("Missing exchange in {}", table_name)
                })?
                .to_string();
            let monitor_enabled = config_map.get("monitor_enabled")
                .map(|v| v == "1")
                .unwrap_or(false);

            let bot_config = BotConfig {
                account_id,
                exchange,
                monitor_enabled,
            };

            configs.insert(applicant, bot_config);
        }

        info!("Loaded {} bot configs", configs.len());
        Ok(Self { configs })
    }
}