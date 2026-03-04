# CryptoGP

Real-time cryptocurrency price visualizer where token performance drives a Formula 1-style 3D race. Instead of charts and candles, price movements are represented as low-poly F1 cars racing around a circuit — tokens with higher percentage gains move faster.

## How It Works

```
CoinGecko REST API                    React Frontend
    |                                      ^
    | poll every 30s                       | WebSocket (500ms)
    v                                      |
 coingecko.rs --> PriceStore --> PriceEngine --> Broadcast Loop --> ws_server.rs
```

1. **Data ingestion** — The backend polls CoinGecko's `/simple/price` REST endpoint every 30 seconds, fetching USD prices for all configured coins in a single HTTP request.
2. **Price engine** — Each price tick is stored in a rolling `VecDeque` per coin (pruned to 24h). The engine computes percentage change over the selected time window by comparing the latest price against the oldest tick within that window.
3. **Speed normalization** — Percentage changes are mapped linearly to a speed range of `[0.3, 1.5]`, so even the worst performer still creeps forward and the leader doesn't instantly lap everyone.
4. **Position accumulation** — Every 500ms broadcast tick, each car's position advances by `speed * dt`. Position wraps at `1.0` to simulate lapping. This keeps the animation smooth — cars move continuously even though price data only updates every 30 seconds.
5. **WebSocket broadcast** — The serialized `RaceState` JSON is pushed to all connected frontend clients every 500ms. Dead connections are automatically pruned via `retain`.

## Tech Stack

| Layer | Technology | Purpose |
|-------|-----------|---------|
| **Backend runtime** | Rust + Tokio | Async runtime for concurrent tasks |
| **HTTP server** | Axum (with `ws` feature) | WebSocket server for frontend clients |
| **HTTP client** | Reqwest | Polling CoinGecko REST API |
| **Serialization** | Serde + serde_json | JSON for API responses and WS broadcast |
| **Logging** | tracing + tracing-subscriber | Structured logging with env-filter |
| **Config** | dotenvy | Load `.env` variables at startup |
| **Timestamps** | chrono | DateTime handling for rolling windows |
| **Frontend framework** | React 18 + Vite | SPA with hot reload |
| **3D rendering** | React Three Fiber + drei | Three.js via declarative JSX |
| **State management** | Zustand | Lightweight global store |
| **Styling** | Tailwind CSS 3 | Utility-first CSS for HUD overlay |
| **3D assets** | GLTF (.glb) | Low-poly track and car models |
| **Price data** | CoinGecko REST API | Free tier, 30s cache TTL |

## Why CoinGecko (Not Binance)

Binance is banned in Indonesia. CoinGecko is globally accessible and provides aggregated multi-exchange pricing. CoinGecko's WebSocket API requires the paid Analyst tier ($129/mo), so we use REST polling on the free/Demo tier instead. A free Demo API key gives a stable 30 req/min — we only use ~2 req/min at 30s intervals since all coins are fetched in one request.

We don't use CoinGecko's pre-baked `price_change_percentage` field. Instead, we store raw price snapshots in `PriceStore` and compute percentage change ourselves over whichever time window the user selects (1m / 5m / 15m / 1h / 24h).

## Project Structure

```
CryptoRace/
├── backend/
│   ├── .env                    # Runtime config (coin IDs, API key, intervals)
│   ├── Cargo.toml              # Rust dependencies
│   └── src/
│       ├── main.rs             # Entry point — spawns poller, broadcast loop, HTTP server
│       ├── config.rs           # Config struct loaded from .env via dotenvy
│       ├── models.rs           # PriceTick, CarState, RaceState structs (serde)
│       ├── coingecko.rs        # REST poller — polls /simple/price, pushes ticks to PriceStore
│       ├── price_engine.rs     # PriceStore + percent change + speed normalization
│       └── ws_server.rs        # Axum router, /ws handler, client registry
├── frontend/
│   ├── package.json            # React, R3F, drei, Zustand, Tailwind
│   ├── vite.config.ts          # Vite + WS proxy to backend
│   └── src/
│       ├── main.tsx            # React entry point
│       └── App.tsx             # Root component (Scene + HUD)
├── shared/
│   └── protocol.ts             # WebSocket JSON contract reference
├── todo.md                     # Full 40-step build roadmap
└── fileExplain.md              # Per-file documentation
```

## Backend Deep Dive

### `config.rs` — Configuration

Loads all runtime settings from environment variables / `.env`:

- `COIN_IDS` — comma-separated CoinGecko coin IDs (e.g. `bitcoin,ethereum,solana`)
- `PORT` — Axum server port (default `9001`)
- `BROADCAST_INTERVAL_MS` — how often to push race state to clients (default `500`)
- `POLL_INTERVAL_MS` — how often to hit CoinGecko (default `30000`)
- `DEFAULT_TIME_WINDOW` — `1m | 5m | 15m | 1h | 24h` (default `5m`)
- `COINGECKO_API_KEY` — optional Demo key for stable 30 req/min

The `Config` struct is created once at startup and shared across tasks via `Arc<Config>`.

### `coingecko.rs` — Price Data Poller

Runs as a long-lived `tokio::spawn` task. Uses a single `reqwest::Client` (connection pooling) to poll:

```
GET https://api.coingecko.com/api/v3/simple/price
  ?ids=bitcoin,ethereum,solana,...
  &vs_currencies=usd
  &include_last_updated_at=true
```

Response shape:
```json
{
  "bitcoin":  { "usd": 67187.34, "last_updated_at": 1711356300 },
  "ethereum": { "usd": 3521.10,  "last_updated_at": 1711356300 }
}
```

Each entry is converted to a `PriceTick` and pushed into the shared `PriceStore`. Error handling:
- **HTTP 429 (rate limited)** — waits 60 seconds, then retries
- **Other errors** — exponential backoff starting at 5s, doubling up to 60s cap

### `price_engine.rs` — Rolling Windows & Speed Math

**PriceStore** is a `HashMap<String, VecDeque<PriceTick>>` wrapped in `Arc<Mutex<>>`. Each `push_tick` appends to the deque and prunes entries older than 24 hours from the front.

**Percent change computation:**
1. Get the deque for the symbol
2. Find the oldest tick within (or before) the requested time window
3. If no tick is old enough, fall back to the oldest available tick (graceful startup)
4. `(current - open) / open * 100.0`

**Speed normalization:**
1. Compute percent change for all tracked coins
2. Find the min and max across all values
3. Map each linearly to `[0.3, 1.5]` — if all changes are equal, everyone gets `0.9`

**Car colors** are assigned from a hardcoded palette, cycling by index.

### `ws_server.rs` — WebSocket Server

Uses Axum's built-in WebSocket support with a channel-per-client architecture:

1. Client connects to `GET /ws` → HTTP upgrade to WebSocket
2. An `mpsc::unbounded_channel` is created for the client
3. The `Sender` is registered in the `ClientRegistry` (`Arc<Mutex<Vec<Sender>>>`)
4. A forwarding task reads from the channel receiver and writes to the WS sink
5. A read loop watches for client disconnect

The broadcast loop in `main.rs` iterates the registry every 500ms and sends the serialized `RaceState` JSON. `clients.retain(|tx| tx.send(...).is_ok())` automatically removes disconnected clients.

CORS is configured via `tower-http::CorsLayer` to allow any origin.

### `main.rs` — Orchestration

Spawns three concurrent components:

1. **CoinGecko poller task** — `coingecko::run()` — writes to `PriceStore`
2. **Broadcast loop task** — reads `PriceStore`, computes `RaceState`, accumulates positions (`pos += speed * dt`, wraps at 1.0), serializes to JSON, pushes to all clients via the registry. Handles NaN/Infinity guards on speed values.
3. **Axum HTTP server** — `axum::serve()` on `0.0.0.0:{PORT}` — handles WebSocket upgrades

All three share state through `Arc<Mutex<>>` pointers to the `PriceStore` and `ClientRegistry`.

## WebSocket Protocol

**Server → Client** (every 500ms):
```json
{
  "timestamp": "2024-01-15T10:30:00.123Z",
  "time_window": "5m",
  "cars": [
    {
      "symbol": "bitcoin",
      "display_name": "bitcoin",
      "price": 63241.50,
      "percent_change": 2.34,
      "speed": 1.12,
      "position": 0.73,
      "color_hex": "#e63946"
    }
  ]
}
```

| Field | Type | Description |
|-------|------|-------------|
| `timestamp` | ISO 8601 | Server time of computation |
| `time_window` | `"1m"` \| `"5m"` \| `"15m"` \| `"1h"` \| `"24h"` | Active window |
| `percent_change` | `f64` | `+` up, `-` down, relative to window open |
| `speed` | `f64` | Normalized to `[0.3, 1.5]` |
| `position` | `f64` | `0.0` = start, wraps at `1.0` (lapping) |

## Getting Started

### Prerequisites

- **Rust** (stable, 2021 edition) — [rustup.rs](https://rustup.rs)
- **Node.js** (18+) and npm
- **CoinGecko Demo API key** (free) — [coingecko.com/en/api/pricing](https://www.coingecko.com/en/api/pricing)

### Backend

```bash
cd backend
cp .env.example .env   # or edit .env directly
# Set your COINGECKO_API_KEY in .env
cargo run
```

The server starts on `http://localhost:9001`. Test the WebSocket in a browser console:

```js
const ws = new WebSocket('ws://localhost:9001/ws');
ws.onmessage = e => console.log(JSON.parse(e.data));
```

You should see `RaceState` JSON arriving every 500ms. Prices will be 0 for the first ~30s until the first CoinGecko poll completes.

### Frontend

```bash
cd frontend
npm install
npm run dev
```

Vite dev server starts on `http://localhost:5173` with a WebSocket proxy to the backend.

### Configuration

All settings are in `backend/.env`:

```env
PORT=9001
COIN_IDS=bitcoin,ethereum,solana,binancecoin,cardano,dogecoin,ripple,avalanche-2,polkadot,chainlink
DEFAULT_TIME_WINDOW=5m
BROADCAST_INTERVAL_MS=500
POLL_INTERVAL_MS=30000
COINGECKO_API_KEY=your-demo-key-here
```

## Cargo Dependencies

| Crate | Why |
|-------|-----|
| `tokio` (full) | Async runtime — runs poller, broadcaster, and HTTP server concurrently |
| `reqwest` (json) | HTTP client for CoinGecko REST polling |
| `axum` (ws) | HTTP + WebSocket server for frontend connections |
| `tower` / `tower-http` (cors) | Middleware layer for CORS |
| `serde` / `serde_json` | Serialize/deserialize JSON for API + WebSocket messages |
| `futures-util` | Async stream combinators for WebSocket sink/stream splitting |
| `dotenvy` | Load `.env` file into environment |
| `tracing` / `tracing-subscriber` | Structured logging with `info!`, `warn!`, `error!` macros |
| `chrono` (serde) | Timestamps for price snapshots and rolling window math |

## Build Status

The backend is fully functional (Phases 1–4 complete). The frontend is scaffolded with React + Vite + Tailwind but the 3D scene, HUD, and WebSocket integration are still in progress. See `todo.md` for the full 40-step build roadmap.
