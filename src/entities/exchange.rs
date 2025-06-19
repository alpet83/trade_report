use tracing::{info};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Serialize, Deserialize};

use crate::entities::candle::{Candle, Tick};

#[derive(Debug, Serialize, Deserialize)]
pub struct Exchange {
    pub name: String,
    #[serde(skip)]
    pub candle_cache: DashMap<(String, DateTime<Utc>), Candle>,
    #[serde(skip)]
    pub tick_cache: DashMap<(String, DateTime<Utc>), Tick>,
}

impl Exchange {
    pub fn new(name: String) -> Self {
        Self {
            name,
            candle_cache: DashMap::new(),
            tick_cache: DashMap::new(),
        }
    }

    pub fn default() -> Self {
        Self {
            name: String::new(),
            candle_cache: DashMap::new(),
            tick_cache: DashMap::new(),
        }
    }

    // Placeholder for loading public data (candles, ticks)
    pub async fn load_public_data(&mut self) -> Result<(), String> {
        // TODO: Implement loading of candles and ticks
        info!("Loading public data for exchange {}", self.name);
        Ok(())
    }
}