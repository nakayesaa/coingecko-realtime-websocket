// This file is responsible for fetching live crypto prices from CoinGecko
// and converting them into PriceTick structs that we already define
// and the rest of the backend can use

//atomically refrence counted pointer, so multiple tasks can share the same config without owning it
use std::sync::Arc;
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::Deserialize;
use crate::config::Config;
use crate::models::PriceTick;

#[derive(Deserialize)]
struct CoinGeckoPrice {
    usd: f64,
    last_updated_at: i64,
}

pub async fn poll_once(
    client: &Client,
    config: &Config,
) -> Result<Vec<PriceTick>, reqwest::Error> {

    let ids = config.coin_ids.join(",");
    let mut req = client
        .get("https://api.coingecko.com/api/v3/simple/price")
        .query(&[
            ("ids", ids.as_str()),
            ("vs_currencies", "usd"),
            ("include_last_updated_at", "true"),
        ]);
        
    if let Some(key) = &config.coingecko_api_key {
        req = req.header("x-cg-demo-api-key", key);
    }

    // Each step returns a Result — the `?` at the end of each line, so if theres an error, return immediately:
    let resp = req
        .send()
        .await?
        .error_for_status()?
        .json::<HashMap<String, CoinGeckoPrice>>()
        .await?;

    let ticks = resp
        .into_iter()
        .map(|(coin_id, data)|{
            let timestamp = DateTime::from_timestamp(data.last_updated_at, 0)
                .unwrap_or_else(Utc::now);
            PriceTick {
                symbol: coin_id,
                price: data.usd,
                timestamp,
            }
        })
        .collect();

    Ok(ticks)
}

pub async fn run(config: Arc<Config>) {
    // Create the HTTP client once here and reuse it every poll.
    let client = Client::new();

    // backoff_secs tracks how long to wait before retrying after a failure
    let mut backoff_secs = 5u64;
    loop{
        // Try to fetch prices. match handles both the Ok and Err cases explicitly.
        match poll_once(&client, &config).await {
            Ok(ticks) => {
                backoff_secs = 5;
                for tick in &ticks {
                    tracing::info!("{}: ${:.2}  ({})", tick.symbol, tick.price, tick.timestamp);
                }

                tracing::info!(
                    "Polled {} coins from CoinGecko — next poll in {}s",
                    ticks.len(),
                    config.poll_interval_ms / 1000
                );

                // TODO: push each tick into the shared PriceStore so
                // the price engine can compute rolling % change from the history.
                tokio::time::sleep(tokio::time::Duration::from_millis(config.poll_interval_ms)).await;
            }
            Err(e) => {
                if e.status().map(|s| s.as_u16()) == Some(429) {
                    tracing::warn!("CoinGecko rate limited, wait again in 60s");
                    //sleep the task for 60s, then continue to retry
                    tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
                    continue;
                }
                tracing::warn!(
                    "CoinGecko poll failed: {}, retrying in {}s",
                    e,
                    backoff_secs
                );
                tokio::time::sleep(tokio::time::Duration::from_secs(backoff_secs)).await;
                backoff_secs = (backoff_secs * 2).min(60);
            }
        }
    }
}
