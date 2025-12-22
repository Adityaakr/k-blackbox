# Kraken Blackbox

**A Rust SDK for Kraken WebSocket v2 market data with verified L2 orderbooks and deterministic replay**

- Verified L2 orderbooks with checksum mismatch detection + auto resync
- Flight recorder that captures raw WS frames and replays them deterministically
- Health + metrics + exportable bug bundles for incident debugging

---

## Quickstart

```bash
cargo build --release
./target/release/blackbox run --symbols BTC/USD --depth 10
curl http://127.0.0.1:8080/health
curl http://127.0.0.1:8080/book/BTC%2FUSD/top
```

---

## What problem it solves

**Before**: Building trading systems on Kraken WebSocket v2 requires manual WebSocket handling, orderbook state management, and checksum verification. When orderbook bugs occur, there's no way to reproduce them deterministically. Developers spend hours debugging production issues with no visibility into what went wrong.

**After**: Kraken Blackbox provides a production-minded Rust SDK that abstracts connection complexity, automatically verifies data integrity via CRC32 checksums, and enables deterministic bug reproduction through recording and replay. Teams can trust their orderbooks and debug issues with complete frame-level visibility.

---

## What we built

**Primary deliverable**: A Rust SDK crate (`blackbox-core` + `blackbox-ws`) for consuming Kraken WebSocket v2 public market data channels. The SDK maintains verified L2 orderbooks, validates CRC32 checksums per Kraken's v2 specification, and provides recording/replay capabilities.

**Example application**: A CLI tool (`blackbox-server`) demonstrates SDK usage with a local HTTP API for health, orderbook queries, and bug bundle export.

The SDK connects to `wss://ws.kraken.com/v2`, subscribes to `instrument` (to get `price_precision`/`qty_precision`) and `book` (L2 snapshot + updates), validates CRC32 checksums from book messages with auto-resync on mismatch, handles keepalive (ping/heartbeat) + reconnection backoff + exceeded msg rate detection, and records raw WebSocket frames with timestamps for deterministic replay.

---

## Features

- **Checksum-verified orderbooks**: CRC32 validation per Kraken WS v2 spec, auto-resync on mismatch
- **Deterministic record/replay**: Records raw WS frames + decoded events, replays through same pipeline
- **Precision-preserving decimals**: Uses `rust_decimal::Decimal` throughout (no floating-point errors)
- **Health monitoring**: Per-symbol metrics, checksum success rates, connection status
- **Bug bundle export**: One-click ZIP export (config, health, frames, orderbook state)
- **Robust WebSocket client**: Auto-reconnection with exponential backoff, rate limit handling
- **Production-minded design**: Lock-free reads, efficient BTreeMap orderbooks, zero-copy parsing

---

## SDK usage

```rust
use blackbox_ws::{WsClient, WsEvent};
use blackbox_core::{Orderbook, verify_checksum, InstrumentInfo};
use tokio::sync::mpsc;
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let client = WsClient::new(
        vec!["BTC/USD".to_string()],
        10, // depth
        Duration::from_secs(30),
        tx,
    );

    tokio::spawn(async move {
        client.run().await.unwrap();
    });

    let mut orderbooks = std::collections::HashMap::new();
    let mut instruments = std::collections::HashMap::new();

    while let Some(event) = rx.recv().await {
        match event {
            WsEvent::InstrumentSnapshot(inst_map) => {
                instruments = inst_map;
            }
            WsEvent::BookSnapshot { symbol, bids, asks, checksum } => {
                let mut ob = Orderbook::new();
                ob.apply_snapshot(bids, asks);
                orderbooks.insert(symbol.clone(), ob);
                
                if let Some(expected) = checksum {
                    let inst = instruments.get(&symbol).unwrap();
                    let computed = verify_checksum(&orderbooks[&symbol], inst)?;
                    if computed != expected {
                        eprintln!("Checksum mismatch for {}: {} != {}", symbol, computed, expected);
                    }
                }
            }
            WsEvent::BookUpdate { symbol, bids, asks, checksum, .. } => {
                let ob = orderbooks.get_mut(&symbol).unwrap();
                ob.apply_updates(bids, asks);
                
                if let Some(expected) = checksum {
                    let inst = instruments.get(&symbol).unwrap();
                    let computed = verify_checksum(ob, inst)?;
                    if computed != expected {
                        eprintln!("Checksum mismatch: {} != {}", computed, expected);
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}
```

---

## CLI usage

**Run live connection:**
```bash
./target/release/blackbox run \
  --symbols BTC/USD,ETH/USD \
  --depth 25 \
  --http 127.0.0.1:8080 \
  --record ./session.ndjson
```

**Replay recording:**
```bash
./target/release/blackbox replay \
  --input ./session.ndjson \
  --speed 4.0 \
  --http 127.0.0.1:8080
```

---

## HTTP API

- `GET /health` - Overall health + per-symbol metrics
- `GET /book/:symbol/top` - Top of book (best bid/ask, spread, mid)
- `GET /book/:symbol?limit=N` - Full orderbook (or limited depth)
- `GET /metrics` - Prometheus-formatted metrics
- `POST /export-bug` - Export bug bundle ZIP (config, health, frames, instruments)

See `/docs/api.md` for detailed examples.

---

## Recording & replay

The recorder writes NDJSON with raw WebSocket frames + timestamps. The replayer re-feeds frames through the same pipeline (orderbook updates, checksum verification) deterministically. This enables "time travel" debugging: reproduce production bugs exactly, test fixes against recorded incidents, and share bug bundles with teams or Kraken support.

Replay modes: realtime, speed multiplier (e.g., 4x), or as-fast-as-possible.

---

## Checksum verification

Implements CRC32 checksum exactly per [Kraken's v2 checksum guide](https://docs.kraken.com/api/docs/guides/spot-ws-book-v2/). The algorithm formats price/qty as fixed decimals (using `price_precision`/`qty_precision` from instrument channel), concatenates asks then bids, and computes CRC32. We avoid floating-point arithmetic entirely—all prices/quantities use `rust_decimal::Decimal` to preserve exact precision. See `/docs/checksum.md` for algorithm details.

---

## Testing

```bash
# Unit tests
cargo test

# Integration test
./test.sh

# Test checksum verification
cargo test --package blackbox-core checksum
```

---

## Repo layout

```
kraken-blackbox/
├── crates/
│   ├── blackbox-core/     # SDK: orderbook, checksum, precision, types
│   ├── blackbox-ws/        # SDK: WebSocket client, parser
│   └── blackbox-server/    # Example app: CLI + HTTP API
├── Cargo.toml
└── README.md
```

---

## Roadmap

- Web UI dashboard for real-time visualization
- Multi-exchange support (Binance, Coinbase)
- Historical replay analysis tools
- Distributed mode with shared state
- Advanced metrics (latency percentiles, spread analysis)

---

## References

- [Kraken WebSocket v2 Book Documentation](https://docs.kraken.com/api/docs/websocket-v2/book)
- [Kraken Checksum Guide (v2)](https://docs.kraken.com/api/docs/guides/spot-ws-book-v2/)
- [Kraken WebSocket FAQ](https://support.kraken.com/articles/360022326871-kraken-websocket-api-frequently-asked-questions)

---

## License

MIT License - see [LICENSE](LICENSE) file for details.
