use std::sync::{Arc, Mutex};

mod config;
mod models;
mod coingecko;
mod price_engine;
mod ws_server;
use config::Config;
use price_engine::Pricestore;

#[tokio::main]
async fn main(){
    tracing_subscriber::fmt().with_env_filter("info").init();
    let config = Config::from_env().expect("Load Config Failed");
    tracing::info!("Load Config : {:?}", config);

    let price_store = Arc::new(Mutex::new(Pricestore::new()));
    let price_store_coingecko = Arc::clone(&price_store);
    let price_store_broadcast = Arc::clone(&price_store);
}