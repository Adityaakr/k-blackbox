# Kraken Blackbox
## Verified orderbooks. Reproducible incidents.

**Rust SDK + TUI for Kraken WebSocket v2 that makes orderbook integrity *observable* (CRC32) and bugs *replayable* (NDJSON).**  
**Includes incident ZIP export with full context + verify latency telemetry (last/avg/p95) inside the TUI.**

[![Rust](https://img.shields.io/badge/Rust-1.70+-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Track: SDK Client](https://img.shields.io/badge/Track-SDK%20Client-blue.svg)]()

> **Positioning:** Throughput SDKs can be fast and still be wrong.  
> **Blackbox proves correctness in real time, and turns production mismatches into deterministic replayable bundles.**

---

### Demo video
https://youtu.be/tK241-jVu-M

### Screenshots
<img width="1021" height="637" alt="TUI" src="https://github.com/user-attachments/assets/c568972a-608a-409e-a02f-7a32dbfc5c2c" />
<img width="1031" height="648" alt="Integrity Inspector" src="https://github.com/user-attachments/assets/63673d26-7f5b-4bab-8dee-ed0316fd84fc" />
<img width="322" height="160" alt="Health" src="https://github.com/user-attachments/assets/38371790-c504-4af7-a0cb-3e498126ce26" />
<img width="330" height="89" alt="Export" src="https://github.com/user-attachments/assets/7502e3db-7d78-4dbd-9ba8-28541cb7ec79" />

---

## What Kraken gives vs what Blackbox adds

Kraken WS v2 includes a **CRC32 checksum** on book updates (computed from top-of-book levels).  
**Blackbox computes the same checksum locally using Kraken’s instrument precisions and verifies it live.**

- Kraken provides: `checksum_expected`
- Blackbox adds: `checksum_computed` + **MATCH/MISMATCH** + **telemetry + replay + incident bundle**

---

## 30-second quickstart

```bash
cargo build --release

# Live TUI (Integrity Inspector)
./target/release/blackbox tui --symbols BTC/USD,ETH/USD --depth 10

# HTTP API (optional)
./target/release/blackbox run --symbols BTC/USD --depth 10 --http 127.0.0.1:8080
curl http://127.0.0.1:8080/health | jq .
````

### What you’ll see in the TUI

* **Expected vs Computed CRC32** side-by-side
* **✅ MATCH / ❌ MISMATCH** integrity status
* Live orderbook + health counters
* **Verify latency telemetry**: last / avg / p95 (typical p95 < 10ms)

---

## The 2-minute judge demo (copy/paste)

### 1) Prove integrity live

```bash
./target/release/blackbox tui --symbols BTC/USD,ETH/USD,SOL/USD --depth 10
```

Show the **Integrity Inspector**:

* Expected checksum (from Kraken)
* Computed checksum (from Blackbox)
* ✅ MATCH badge

### 2) Record frames (NDJSON)

Inside the TUI:

* Press **[R]** to toggle recording (ON)
* Wait ~10–20 seconds
* Press **[R]** again (OFF)

Or via CLI:

```bash
./target/release/blackbox tui --symbols BTC/USD --depth 10 --record session.ndjson
```

### 3) Replay + trigger a controlled mismatch (fault injection)

```bash
./target/release/blackbox tui \
  --symbols BTC/USD --depth 10 \
  --replay session.ndjson \
  --fault mutate_qty \
  --once-at 50 \
  --speed 4.0
```

Watch the Integrity Inspector flip:

* ✅ MATCH → ❌ MISMATCH
* Event log: `FAULT_INJECTED` → `CHECKSUM_MISMATCH` → `INCIDENT_CAPTURED`

### 4) Export incident bundle (ZIP)

In the TUI press **[E]**
or

```bash
curl -X POST http://127.0.0.1:8080/export-bug -o incident.zip
unzip -l incident.zip
```

### 5) Reproduce the incident deterministically

```bash
./target/release/blackbox replay-incident \
  --bundle ./incidents/incident_*.zip \
  --speed 4.0
```

Same mismatch occurs at the same frame → reproducible debugging.

---

## Why this matters (the silent failure problem)

Orderbooks can silently diverge due to missed updates, reconnect edges, precision mistakes, or parsing bugs.
When that happens:

* Logs are incomplete
* The exact frame sequence is gone
* The bug is not reproducible
* Debugging turns into guessing

**Blackbox makes correctness visible and debugging deterministic.**

---

## Key features

### Integrity (the differentiator)

* Live **CRC32 checksum verification** on every book update (Kraken WS v2 spec)
* **Expected vs Computed** checksum display in TUI
* Mismatch event timeline + per-symbol integrity stats
* **Verify latency telemetry**: last / avg / p95

### Record / Replay

* Frame-level recording to **NDJSON**
* Deterministic replay at any speed: realtime / xN / as-fast
* Fault injection for demo + robustness testing:

  * `--fault mutate_qty --once-at N` (and other modes if implemented)

### Incidents

* One-click **incident ZIP export** with full context:

  * checksums (expected/computed + preview string)
  * orderbook snapshot
  * health snapshot
  * recent frames window (NDJSON)
  * config + metadata

### Production-minded SDK choices

* `rust_decimal::Decimal` end-to-end (no float drift)
* `BTreeMap` orderbook structure (ordered iteration for checksum)
* Tokio async WS client + clean event pipeline
* Optional HTTP API for health + export

---

## Architecture (mental model)

```
Kraken WS frames
   ↓
Parser → typed events
   ↓
Orderbook (BTreeMap<Decimal, Decimal>)
   ↓
Checksum verifier (CRC32)
   ├─ MATCH → update health + telemetry
   └─ MISMATCH → capture incident + export bundle
   ↓
Recorder (NDJSON) ↔ Replayer (same pipeline)
   ↓
TUI / HTTP API read shared state
```

---

## Commands

### Live

```bash
./target/release/blackbox tui --symbols BTC/USD,ETH/USD --depth 10
```

### Record

```bash
./target/release/blackbox tui --symbols BTC/USD --depth 10 --record session.ndjson
```

### Replay

```bash
./target/release/blackbox tui --symbols BTC/USD --depth 10 --replay session.ndjson --speed 4.0
```

### Fault injection demo

```bash
./target/release/blackbox tui --symbols BTC/USD --depth 10 --replay session.ndjson --fault mutate_qty --once-at 50
```

### HTTP API

```bash
./target/release/blackbox run --symbols BTC/USD --depth 10 --http 127.0.0.1:8080
curl http://127.0.0.1:8080/health | jq .
```

---

## Incident bundle format

```text
incidents/
└── incident_*.zip
    ├── metadata.json
    ├── config.json
    ├── health.json
    ├── frames.ndjson
    ├── orderbook.json
    └── checksums.json
```

---

## Project structure

```text
crates/
  blackbox-core/     # orderbook + checksum + precision + recorder/replay
  blackbox-ws/       # websocket client + parser + events
  blackbox-server/   # CLI + TUI + HTTP API + incident export
```

---

## References

* Kraken WS v2 book docs: [https://docs.kraken.com/api/docs/websocket-v2/book](https://docs.kraken.com/api/docs/websocket-v2/book)
* Kraken checksum guide (v2): [https://docs.kraken.com/api/docs/guides/spot-ws-book-v2/](https://docs.kraken.com/api/docs/guides/spot-ws-book-v2/)

---

## License

MIT
--- ## 📄 License MIT License - see [LICENSE](LICENSE) file for details. ---

