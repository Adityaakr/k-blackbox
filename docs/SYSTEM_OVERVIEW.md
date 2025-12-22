# System Overview: End-to-End Understanding

Complete guide to understanding how Kraken Blackbox works from top to bottom.

---

## 🎯 The 20% Mental Model (Remember This)

```
1. WS Client reads raw frames →
2. Parser turns frames into events →
3. Orderbook applies snapshot/updates →
4. Checksum verifies local book matches Kraken →
5. Recorder saves frames, Replay replays them →
6. HTTP/Health exposes the state to you
```

---

## 📁 File-by-File Walkthrough (Read Order)

### 1️⃣ Entry Point: How Everything Starts

**File:** `crates/blackbox-server/src/main.rs`

**What it does:**
- Parses CLI arguments (`run` vs `replay`)
- Creates shared state (`AppState`)
- Spawns 3 async tasks:
  1. WebSocket client (reads from Kraken)
  2. Event processor (processes WS events → updates orderbooks)
  3. HTTP server (serves API endpoints)

**Key Functions:**
- `main()` - Entry point, parses CLI
- `run_client()` - Sets up live connection mode
  - Lines 93-170: Creates state, recorder, WS client, processor, HTTP server
  - Uses `tokio::select!` to run all tasks concurrently
- `process_ws_events()` - **THE CORE LOOP** (Lines 172-304)
  - Receives events from WS client via channel
  - Routes events to appropriate handlers
  - Updates orderbooks, verifies checksums, records frames

**Data Flow:**
```
CLI args → main() → run_client() → spawns 3 tasks:
  ├─> WsClient.run() → sends events to channel
  ├─> process_ws_events() → reads from channel, updates state
  └─> HTTP server → reads from state, serves API
```

---

### 2️⃣ WebSocket Client: Where Data Comes From

**File:** `crates/blackbox-ws/src/client.rs`

**What it does:**
- Connects to `wss://ws.kraken.com/v2`
- Sends subscription messages (`instrument`, `book`)
- Receives raw JSON frames in a loop
- Handles reconnection with exponential backoff
- Sends ping messages every 30s (keepalive)
- Parses frames and emits typed events via channel

**Key Functions:**
- `WsClient::new()` - Creates client with symbols, depth, ping interval
- `run()` - **Main loop** (Lines 53-80)
  - Infinite reconnect loop with exponential backoff
  - Calls `connect_and_run()` on each attempt
- `connect_and_run()` - **Connection logic** (Lines 82-341)
  - Connects to WebSocket
  - Subscribes to `instrument` channel first
  - Waits for instrument snapshot
  - Then subscribes to `book` channel
  - Main read loop: receives frames → parses → sends events

**Event Types Emitted:**
```rust
WsEvent::Connected
WsEvent::Disconnected
WsEvent::Frame(String)              // Raw JSON frame
WsEvent::InstrumentSnapshot(...)    // Parsed instrument data
WsEvent::BookSnapshot { ... }       // Parsed book snapshot
WsEvent::BookUpdate { ... }         // Parsed book update
WsEvent::Error(String)
WsEvent::RateLimitExceeded
```

**Data Flow:**
```
Kraken WS → connect_and_run() → read.next() → parse_frame() → 
  ├─> WsEvent::Frame (raw) → channel
  └─> WsEvent::BookSnapshot/Update (parsed) → channel
```

**Key Details:**
- Lines 94-98: Subscribes to instrument channel
- Lines 147-177: Processes instrument snapshot, stores precisions
- Lines 179-180: Subscribes to book channel after instrument received
- Lines 182-241: Processes book messages, parses price/qty as Decimal
- Lines 124-138: Handles ping/heartbeat and rate limit detection

---

### 3️⃣ Message Parser: Traffic Controller

**File:** `crates/blackbox-ws/src/parser.rs`

**What it does:**
- Takes raw JSON string frames
- Reads `channel` and `type` fields
- Converts to normalized Rust enums/structs
- Handles all message types (book, instrument, status, heartbeat, ping)

**Key Function:**
- `parse_frame()` - **The router** (Lines 5-51)
  - Checks if it's an ACK/response
  - Checks `channel` field
  - Routes to appropriate deserializer
  - Returns `WsFrame` enum

**Frame Types:**
```rust
WsFrame::Ack(WsAck)              // Subscription confirmations
WsFrame::Book(BookMessage)        // Book snapshot/update
WsFrame::Instrument(InstrumentMessage)  // Instrument data
WsFrame::Status(StatusMessage)    // System status
WsFrame::Heartbeat(HeartbeatMessage)    // Keepalive
WsFrame::Ping(PingMessage)        // Ping/pong
```

**Data Flow:**
```
Raw JSON string → parse_frame() → WsFrame enum → 
  client.rs converts to WsEvent → sent to channel
```

**Key Details:**
- Lines 8-12: Detects ACK messages (subscription confirmations)
- Lines 15-50: Routes by `channel` field
- Uses `serde_json` for deserialization
- Returns `anyhow::Result` for error handling

---

### 4️⃣ Orderbook Engine: The Brain

**File:** `crates/blackbox-core/src/orderbook.rs`

**What it does:**
- Stores bids and asks in `BTreeMap<Decimal, Decimal>`
- Applies snapshots (replaces all levels)
- Applies updates (incremental changes, removes if qty=0)
- Truncates to configured depth
- Provides accessors (best_bid, best_ask, spread, mid)

**Key Functions:**
- `new()` - Creates empty orderbook
- `apply_snapshot()` - **Replaces all levels** (Lines 23-38)
  - Clears existing bids/asks
  - Inserts new levels (skips qty=0)
- `apply_updates()` - **Incremental updates** (Lines 41-59)
  - If qty=0, removes the level
  - Otherwise, inserts/updates the level
- `truncate()` - **Keeps top N levels** (Lines 62-86)
  - Asks: keep lowest (first) `depth` levels
  - Bids: keep highest (last) `depth` levels
- `best_bid()`, `best_ask()`, `spread()`, `mid()` - Accessors

**Data Structure:**
```rust
pub struct Orderbook {
    asks: BTreeMap<Decimal, Decimal>,  // price -> qty (ascending)
    bids: BTreeMap<Decimal, Decimal>,  // price -> qty (ascending, iterated reverse)
}
```

**Why BTreeMap?**
- Ordered iteration (needed for checksum)
- O(log n) insert/remove
- Efficient truncation (can skip/remove ranges)

**Data Flow:**
```
BookSnapshot → apply_snapshot() → orderbook state
BookUpdate → apply_updates() → orderbook state
HTTP request → best_bid()/best_ask() → JSON response
```

**Usage in main.rs:**
- Line 214: `book.apply_snapshot(bids, asks)` - Initialize from snapshot
- Line 257: `book_entry.apply_updates(bids, asks)` - Update from incremental
- Line 216, 261: `book.truncate(depth)` - Keep only top N levels

---

### 5️⃣ Checksum Verification: The Truth Detector

**File:** `crates/blackbox-core/src/checksum.rs`

**What it does:**
- Implements Kraken's exact CRC32 checksum algorithm
- Formats prices/quantities with exact precision
- Builds checksum string (top 10 asks + top 10 bids)
- Computes CRC32 and compares to Kraken's checksum

**Key Functions:**
- `build_checksum_string()` - **Builds the string** (Lines 10-36)
  - Takes top 10 asks (low→high)
  - Takes top 10 bids (high→low)
  - Formats each price/qty using `format_fixed()` from `precision.rs`
  - Concatenates: `price_str + qty_str` for each level
  - Concatenates all asks, then all bids
- `compute_crc32()` - Computes CRC32 hash (Lines 39-43)
- `verify_checksum()` - **Main verification** (Lines 46-55)
  - Builds checksum string
  - Computes CRC32
  - Compares to expected checksum
  - Returns `bool`

**Checksum Algorithm (Kraken v2 spec):**
1. Take top 10 asks (ascending price)
2. Take top 10 bids (descending price)
3. For each level:
   - Format price with `price_precision` (remove '.', trim zeros)
   - Format qty with `qty_precision` (remove '.', trim zeros)
   - Concatenate: `price_str + qty_str`
4. Concatenate all asks, then all bids
5. Compute CRC32 of the entire string
6. Compare to Kraken's checksum

**Data Flow:**
```
Orderbook state → build_checksum_string() → string →
  compute_crc32() → u32 → compare to Kraken's → bool
```

**Usage in main.rs:**
- Lines 221-226: Verifies checksum on snapshot
- Lines 266-271: Verifies checksum on update
- If mismatch: logs warning, records in health metrics

**Precision Handling:**
- Uses `format_fixed()` from `precision.rs`
- Handles scientific notation (e.g., `1e-8`)
- Preserves exact precision (no floating-point errors)

---

### 6️⃣ Precision Module: Decimal Formatting

**File:** `crates/blackbox-core/src/precision.rs`

**What it does:**
- Formats decimals for checksum (Kraken's exact rules)
- Parses strings to Decimal (handles scientific notation)

**Key Functions:**
- `format_fixed()` - Formats decimal with fixed precision
  - Converts to fixed decimal places
  - Removes decimal point
  - Trims leading zeros
  - Example: `50000.12` with precision 2 → `"5000012"`
- `parse_decimal()` - Parses string to Decimal
  - Handles scientific notation (`1e-8`)
  - Uses `Decimal::from_str()`

**Why it matters:**
- Checksum requires exact string formatting
- Floating-point (`f64`) would break checksums
- Must match Kraken's format exactly

---

### 7️⃣ Recorder: The Dashcam

**File:** `crates/blackbox-core/src/recorder.rs`

**What it does:**
- Writes raw WebSocket frames to NDJSON file
- One frame per line: `{"ts": "...", "raw_frame": "...", "decoded_event": "..."}`
- Buffered writes for performance

**Key Functions:**
- `new()` - Creates recorder, opens file (Lines 14-27)
- `record_frame()` - **Writes frame** (Lines 29-43)
  - Creates `RecordedFrame` with timestamp
  - Serializes to JSON
  - Writes line to file
  - Flushes buffer

**Data Format:**
```json
{"ts":"2024-01-15T10:30:45.123Z","raw_frame":"{\"channel\":\"book\",...}","decoded_event":null}
```

**Usage in main.rs:**
- Line 188: `rec.record_frame(&raw_frame, None)` - Records every raw frame
- Only records if `--record` flag provided

**Ring Buffer:**
- Also stored in `state.last_frames` (Lines 192-196 in main.rs)
- Keeps last 1000 frames in memory
- Used for bug bundle export

---

### 8️⃣ Replayer: The Time Machine

**File:** `crates/blackbox-core/src/replayer.rs`

**What it does:**
- Reads NDJSON recording file
- Replays frames through same pipeline
- Supports 3 modes: realtime, speed multiplier, as-fast-as-possible

**Key Functions:**
- `new()` - Loads frames from file (Lines 18-40)
- `start()` - Starts replay timer (Lines 42-47)
- `next_frame()` - **Returns next frame when ready** (Lines 49-87)
  - Checks timing based on replay mode
  - Returns `None` if should wait
  - Returns `Some(frame)` when ready

**Replay Modes:**
```rust
ReplayMode::Realtime      // Original timing
ReplayMode::Speed(4.0)    // 4x speed
ReplayMode::AsFast        // No delays
```

**Usage in main.rs:**
- Lines 306-372: `replay_recording()` function
- Creates replayer, processes frames through same parser
- Updates orderbooks, verifies checksums (same as live mode)

**Data Flow:**
```
NDJSON file → replayer.next_frame() → parse_frame() → 
  process_ws_events() → same pipeline as live mode
```

---

### 9️⃣ Health Tracker: The Dashboard Backend

**File:** `crates/blackbox-core/src/health.rs`

**What it does:**
- Tracks per-symbol health metrics
- Computes health scores
- Records checksum success/failure rates

**Key Structs:**
- `SymbolHealth` - Per-symbol metrics (Lines 4-16)
  - `total_msgs`, `checksum_ok`, `checksum_fail`
  - `reconnect_count`, `msg_rate_estimate`
  - `last_msg_ts`, `last_checksum_mismatch`
- `OverallHealth` - System-wide health (Lines 108-113)

**Key Functions:**
- `record_checksum_ok()` - Increments success counter
- `record_checksum_fail()` - Increments failure counter, tracks consecutive fails
- `health_score()` - Computes 0-100 score (Lines 35-66)
- `status()` - Returns Ok/Warn/Fail based on score

**Usage in main.rs:**
- Lines 228-242: Updates health on snapshot
- Lines 273-287: Updates health on update
- Line 56: HTTP endpoint reads from health state

---

### 🔟 HTTP API: The Dashboard

**File:** `crates/blackbox-server/src/http.rs`

**What it does:**
- Exposes REST API endpoints
- Reads from shared state (orderbooks, health)
- Serves JSON responses

**Endpoints:**
- `GET /health` - Overall health + per-symbol metrics (Lines 55-58)
- `GET /book/:symbol/top` - Top of book (Lines 60-86)
- `GET /book/:symbol` - Full orderbook (Lines 88-116)
- `GET /metrics` - Prometheus metrics (Lines 118-122)
- `POST /export-bug` - Export bug bundle ZIP (Lines 124-185)

**Key Functions:**
- `router()` - Creates Axum router (Lines 45-53)
- `health_handler()` - Reads from `state.overall_health()`
- `book_top_handler()` - Reads from `state.orderbooks.get()`
- `export_bug_handler()` - Creates ZIP with config, health, frames, instruments

**Data Flow:**
```
HTTP request → handler → state.orderbooks/health.get() → 
  format as JSON → HTTP response
```

---

### 1️⃣1️⃣ Shared State: The Memory

**File:** `crates/blackbox-server/src/state.rs`

**What it does:**
- Holds all shared application state
- Thread-safe (uses `DashMap`, `Arc`, `RwLock`)
- Accessed by both event processor and HTTP server

**Key Struct:**
```rust
pub struct AppState {
    orderbooks: Arc<DashMap<String, Orderbook>>,      // Symbol → Orderbook
    instruments: Arc<DashMap<String, InstrumentInfo>>, // Symbol → Instrument
    health: Arc<DashMap<String, SymbolHealth>>,      // Symbol → Health
    depths: Arc<DashMap<String, u32>>,                // Symbol → Depth
    last_frames: Arc<RwLock<Vec<(DateTime, String)>>>, // Ring buffer
    start_time: Instant,
}
```

**Why DashMap?**
- Lock-free concurrent reads
- Multiple readers, single writer per key
- Perfect for HTTP server (many readers) + processor (writer)

**Key Functions:**
- `new()` - Creates empty state
- `overall_health()` - Aggregates health from all symbols
- `set_depth()`, `get_depth()` - Manages depth per symbol

---

## 🔄 Complete Data Flow

### Live Mode (Run)

```
1. User runs: ./blackbox run --symbols BTC/USD
   ↓
2. main.rs:run_client() spawns 3 tasks
   ↓
3. WsClient connects to wss://ws.kraken.com/v2
   ↓
4. WsClient subscribes to instrument channel
   ↓
5. Kraken sends instrument snapshot
   ↓
6. WsClient parses → WsEvent::InstrumentSnapshot → channel
   ↓
7. process_ws_events() receives event
   ↓
8. Stores instruments in state.instruments (precisions)
   ↓
9. WsClient subscribes to book channel
   ↓
10. Kraken sends book snapshot
    ↓
11. WsClient parses → WsEvent::BookSnapshot → channel
    ↓
12. process_ws_events() receives event
    ↓
13. Creates Orderbook, calls apply_snapshot()
    ↓
14. Truncates to depth
    ↓
15. Verifies checksum (verify_checksum())
    ↓
16. Stores orderbook in state.orderbooks
    ↓
17. Updates health metrics
    ↓
18. (Loop) Kraken sends book updates
    ↓
19. WsClient parses → WsEvent::BookUpdate → channel
    ↓
20. process_ws_events() receives event
    ↓
21. Gets existing orderbook, calls apply_updates()
    ↓
22. Truncates to depth
    ↓
23. Verifies checksum
    ↓
24. Updates health metrics
    ↓
25. HTTP request: GET /book/BTC/USD/top
    ↓
26. http.rs reads from state.orderbooks
    ↓
27. Returns JSON with best_bid, best_ask, spread
```

### Replay Mode

```
1. User runs: ./blackbox replay --input session.ndjson
   ↓
2. main.rs:replay_recording() creates Replayer
   ↓
3. Replayer loads frames from NDJSON file
   ↓
4. Replayer.next_frame() returns frame (with timing)
   ↓
5. parse_frame() parses frame (same as live)
   ↓
6. process_ws_events() processes (same as live)
   ↓
7. Orderbook updates, checksums verified (same as live)
   ↓
8. HTTP server serves same endpoints (same as live)
```

---

## 🎯 Key Insights

### Why This Architecture?

1. **Event-Driven**: WS client emits events, processor consumes them
   - Decouples I/O from business logic
   - Easy to add replay mode (same processor, different source)

2. **Shared State**: DashMap for concurrent access
   - HTTP server reads, processor writes
   - No locks needed for reads

3. **Precision Matters**: All prices/quantities use `Decimal`
   - Floating-point would break checksums
   - Must match Kraken's format exactly

4. **Checksum Verification**: Happens on every update
   - Catches data corruption immediately
   - Auto-resync on mismatch (future enhancement)

5. **Recording/Replay**: Same pipeline for both
   - Deterministic reproduction
   - Test fixes against recorded bugs

---

## 📚 Reading Order for Deep Dive

1. **Start here:** `main.rs` (lines 172-304) - `process_ws_events()`
   - This is where everything comes together
   - Shows how events flow through the system

2. **Then:** `client.rs` (lines 82-341) - `connect_and_run()`
   - See how frames are received and parsed
   - Understand the WebSocket loop

3. **Then:** `orderbook.rs` - All functions
   - Understand the data structure
   - See how snapshots vs updates work

4. **Then:** `checksum.rs` - All functions
   - Understand the verification algorithm
   - See why precision matters

5. **Finally:** `http.rs` and `state.rs`
   - See how state is exposed via API
   - Understand concurrent access patterns

---

## 🧪 Testing Your Understanding

1. **Trace a book update:**
   - Start from `client.rs` line 182 (receives frame)
   - Follow through parser, event emission, processor, orderbook update

2. **Trace a checksum verification:**
   - Start from `main.rs` line 266 (verify_checksum call)
   - Follow through `checksum.rs`, `precision.rs`, back to health update

3. **Trace an HTTP request:**
   - Start from `http.rs` line 60 (book_top_handler)
   - Follow through state access, orderbook read, JSON serialization

---

## 🎓 Summary

**The system is a pipeline:**

```
Kraken WS → Client → Parser → Events → Processor → Orderbook → Checksum → Health → HTTP
                                                              ↓
                                                          Recorder → File
                                                              ↑
                                                          Replayer ← File
```

**Key files:**
- `main.rs` - Orchestrates everything
- `client.rs` - Reads from Kraken
- `parser.rs` - Converts JSON to events
- `orderbook.rs` - Maintains state
- `checksum.rs` - Verifies correctness
- `http.rs` - Exposes state

**Everything else is supporting infrastructure:**
- `recorder.rs`, `replayer.rs` - Recording/replay
- `health.rs` - Metrics tracking
- `state.rs` - Shared memory
- `precision.rs` - Decimal formatting

You now understand the entire system! 🎉

