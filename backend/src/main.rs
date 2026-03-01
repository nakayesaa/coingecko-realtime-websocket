mod config;
mod models;
mod coingecko;
mod price_engine;
mod ws_server;
use config::Config;

fn main(){
    tracing_subscriber::fmt().with_env_filter("info").init();
    let config = Config::from_env().expect("Load Config Failed");
    tracing::info!("Load Config : {:?}", config);
}