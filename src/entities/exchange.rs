// /src/entities/exchange.rs
// Modified: 2025-06-22 11:45:00 EEST

use serde::{Serialize, Deserialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;
use chrono::{Utc, Duration};
use tracing::{info, error};

use crate::{
    entities::public_data::Candle,
    entities::cache::PriceCache,
    db::mysql::MySqlDataSource,
    common::consts::BTC_PAIR_ID,
};

// Represents an exchange with cached price and candle data
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Exchange {
    pub name: String,
    #[serde(skip)]
    pub candles_cache: HashMap<String, Vec<Candle>>,
    #[serde(skip)]
    pub price_caches: Arc<RwLock<HashMap<i32, Arc<PriceCache>>>>,
}

impl Exchange {
    // Creates a new Exchange instance with the given name and prefetches BTC price cache
    pub async fn new(name: String) -> Self {
        let exchange = Exchange {
            name,
            candles_cache: HashMap::new(),
            price_caches: Arc::new(RwLock::new(HashMap::new())),
        };

        // Prefetch BTC price cache for the past year
        let cache = exchange.get_price_cache(Some(BTC_PAIR_ID)).await;
        let db = MySqlDataSource::db_conn();
        let end_ts = Utc::now();
        let start_ts = end_ts - Duration::days(365);
        if let Err(e) = cache.load_prefetch(start_ts, end_ts).await {
            error!("Failed to prefetch BTC price cache for exchange {}: {}", exchange.name, e);
        } else {
            info!("Successfully prefetched BTC price cache for exchange {}", exchange.name);
        }

        exchange
    }

    // Retrieves or creates a PriceCache for a given pair ID
    pub async fn get_price_cache(&self, pair_id: Option<i32>) -> Arc<PriceCache> {
        let mut caches = self.price_caches.write().await;
        let cache = caches.entry(pair_id.unwrap_or(0)).or_insert_with(|| {
            Arc::new(PriceCache::new(Arc::new(self.clone()), pair_id))
        });
        cache.clone()
    }
}