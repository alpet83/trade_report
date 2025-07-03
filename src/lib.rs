pub mod common;
pub mod logs;
pub mod entities;
pub mod services;
pub mod db;
pub mod api;
pub mod rtm_notify;
pub mod config;

#[cfg(test)]
mod tests {
    mod setup;
    mod task {
        mod basic;
        mod aggr_trades;
    }
    mod trades_cache;    
    mod interval_func;
    mod basic_logger;
}