use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceTick {
    pub symbol: String,
    pub price: f64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CarState {
    pub symbol: String,
    pub display_name: String,
    pub price: f64,
    pub percent_change: f64,
    pub speed: f64,
    pub position: f64,
    pub color_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaceState {
    pub timestamp: DateTime<Utc>,
    pub time_window: String,
    pub cars: Vec<CarState>,
}
