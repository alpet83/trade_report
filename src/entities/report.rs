use serde::{Serialize, Deserialize};
use sqlx::FromRow;

#[derive(Serialize, Deserialize, FromRow, Debug)]
pub struct ReportConfig {
    pub account_id: i32,
    pub param: String,
    pub value: String,
}

