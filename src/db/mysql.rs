// /src/db/mysql.rs
// Modified: 2025-06-21 14:00:00 EEST

use async_trait::async_trait;
use sqlx::{MySqlPool, Error as SqlxError};
use once_cell::sync::OnceCell;
use std::sync::Arc;

// Static singleton for MySqlDataSource
static DB_CONN: OnceCell<Arc<MySqlDataSource>> = OnceCell::new();


pub fn trading_table_name(exchange: &str, table_suffix: &str) -> String {
    format!("{}__{}", exchange, table_suffix).to_lowercase()
}

pub fn public_table_name(exchange: &str, table_suffix: &str) -> String {
    format!("{}.{}", exchange, table_suffix).to_lowercase()
}

/// Macro to build a SQL query by formatting the base query with the table name and appending conditions.
/// Note: `$base_query` must contain a '{}' placeholder for the table name.
#[macro_export]
macro_rules! build_query {
    ($table:expr, $base_query:expr, $($condition:expr),*) => {
        {
            let mut query = format!($base_query, $table);
            query.push_str(" ");
            $(
                query.push_str($condition);
            )*
            query
        }
    };
}

#[derive(Debug)]
pub struct MySqlDataSource {
    pub pool: MySqlPool,
}

impl MySqlDataSource {
    pub async fn new(url: &str) -> Result<Self, SqlxError> {
        let pool = MySqlPool::connect(url).await?;
        Ok(MySqlDataSource { pool })
    }
    // Retrieves the global MySqlDataSource singleton
    pub fn db_conn() -> Arc<MySqlDataSource> {
        DB_CONN.get().expect("Database connection not initialized").clone()
    }

    // Initializes the global MySqlDataSource singleton
    pub async fn init_db_conn(url: &str) -> Result<(), SqlxError> {
        let db = MySqlDataSource::new(url).await?;
        DB_CONN.set(Arc::new(db)).expect("Database connection already initialized");
        Ok(())
    }

    // Initializes the global MySqlDataSource singleton with a mock for testing
    #[cfg(test)]
    pub async fn init_db_conn_with_mock(db: Arc<MySqlDataSource>) {
        DB_CONN.set(db).expect("Database connection already initialized");
    }
}

