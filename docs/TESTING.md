# Testing Guide

Comprehensive testing guide for Kraken Blackbox, covering unit tests, integration tests, manual verification, TUI testing, and incident bundle replay.

---

## Quick Start

Run the automated test script:

```bash
./test.sh
```

This comprehensive script:
1. Builds the project in release mode
2. Tests TUI in mock mode
3. Tests HTTP API endpoints (health, orderbook, export-bug)
4. Tests recording functionality
5. Tests replay functionality
6. Tests fault injection
7. Tests incident replay (if bundles exist)
8. Runs unit tests
9. Reports results

**Expected output:**
```
╔════════════════════════════════════════════════════════════╗
║     Kraken Blackbox - Comprehensive Test Suite            ║
╚════════════════════════════════════════════════════════════╝

✓ Build successful
✓ Mock mode test completed
✓ Health endpoint working
✓ Book endpoint working
...
```

**Time:** ~2-3 minutes (includes build time)

---

## Unit Tests

### Run All Unit Tests

```bash
cargo test
```

### Run Tests for Specific Crate

```bash
# Test core library
cargo test --package blackbox-core

# Test WebSocket client
cargo test --package blackbox-ws

# Test server
cargo test --package blackbox-server
```

### Run Specific Test

```bash
# Test checksum verification
cargo test --package blackbox-core checksum

# Test orderbook operations
cargo test --package blackbox-core orderbook

# Test precision formatting
cargo test --package blackbox-core precision
```

### Run Tests with Output

```bash
# Show println! output
cargo test -- --nocapture

# Run tests in parallel (default)
cargo test -- --test-threads=1

# Run a single test
cargo test test_kraken_example_checksum -- --nocapture
```

---

## Integration Tests

### Automated Integration Test

```bash
./test.sh
```

**What it tests:**
- Build process
- TUI mock mode
- HTTP API server startup
- WebSocket connection
- Health endpoint
- Top of book endpoint
- Full orderbook endpoint
- Export-bug endpoint
- Recording functionality
- Replay functionality
- Fault injection
- Incident replay

### Manual Integration Test

#### Step 1: Start Server

```bash
./target/release/blackbox run \
  --symbols BTC/USD \
  --depth 10 \
  --http 127.0.0.1:8080
```

Wait 15-20 seconds for connection to establish.

#### Step 2: Test Health Endpoint

```bash
curl http://127.0.0.1:8080/health | python3 -m json.tool
```

**Or with jq:**
```bash
curl -s http://127.0.0.1:8080/health | jq .
```

**Expected:**
- `"status": "OK"`
- `"connected": true`
- `"total_msgs"` > 0
- `"checksum_ok"` > 0
- `"checksum_fail": 0` (or very low)

#### Step 3: Test Top of Book

```bash
curl http://127.0.0.1:8080/book/BTC%2FUSD/top | python3 -m json.tool
```

**Expected:**
- Valid price/quantity pairs
- `best_bid` < `best_ask` (positive spread)
- Prices are reasonable (not 0 or negative)

#### Step 4: Test Full Orderbook

```bash
curl "http://127.0.0.1:8080/book/BTC%2FUSD?limit=5" | python3 -m json.tool
```

**Expected:**
- Array of bids (descending price)
- Array of asks (ascending price)
- At least 1 level on each side

#### Step 5: Test Metrics

```bash
curl http://127.0.0.1:8080/metrics
```

**Expected:** Prometheus-formatted output (or placeholder message)

---

## Functional Tests

### Test Multiple Symbols

```bash
# Start with multiple symbols
./target/release/blackbox run \
  --symbols BTC/USD,ETH/USD,SOL/USD,AVAX/USD \
  --depth 25 \
  --http 127.0.0.1:8080

# Test each symbol (wait 20 seconds first)
curl http://127.0.0.1:8080/book/BTC%2FUSD/top
curl http://127.0.0.1:8080/book/ETH%2FUSD/top
curl http://127.0.0.1:8080/book/SOL%2FUSD/top
curl http://127.0.0.1:8080/book/AVAX%2FUSD/top
```

**Verify:**
- All symbols show `"connected": true` in health
- Each symbol has valid orderbook data
- No errors in logs

### Test Recording

```bash
# Start with recording enabled
./target/release/blackbox run \
  --symbols BTC/USD \
  --depth 10 \
  --http 127.0.0.1:8080 \
  --record ./test-recording.ndjson

# Let it run for 30 seconds, then stop (Ctrl+C)

# Verify recording file exists and has data
wc -l test-recording.ndjson
head -3 test-recording.ndjson
```

**Expected:**
- File exists
- Contains NDJSON lines (one per frame)
- Each line is valid JSON with `ts` and `raw_frame` fields

### Test Replay

```bash
# Replay a recording
./target/release/blackbox replay \
  --input ./test-recording.ndjson \
  --speed 2.0 \
  --http 127.0.0.1:8081

# In another terminal, test endpoints
curl http://127.0.0.1:8081/health
curl http://127.0.0.1:8081/book/BTC%2FUSD/top
```

**Verify:**
- Replay processes frames
- Orderbook state is recreated
- Checksums are verified during replay

### Test Fault Injection

```bash
# Replay with fault injection (mutate qty at frame 50)
./target/release/blackbox replay \
  --input ./test-recording.ndjson \
  --speed 4.0 \
  --http 127.0.0.1:8081 \
  --fault-mutate-once 50 \
  --fault-mutate-delta 1
```

**Verify:**
- Fault is injected at the specified frame
- Checksum mismatch occurs
- Incident is captured

### Test Bug Export

```bash
# While server is running, export incident bundle
curl -X POST http://127.0.0.1:8080/export-bug -o incident.zip

# Extract and verify contents
unzip incident.zip -d incident/
ls -la incident/

# View contents
cat incident/metadata.json | jq .
cat incident/health.json | jq .
head -5 incident/frames.ndjson
```

**Expected:**
- ZIP file created successfully
- Contains files: `metadata.json`, `config.json`, `health.json`, `frames.ndjson`
- Optionally contains: `instrument.json`, `book_top.json`
- All files contain valid JSON/NDJSON

### Test Incident Replay

```bash
# Replay an incident bundle
./target/release/blackbox replay-incident \
  --bundle ./incidents/incident_*.zip \
  --speed 4.0 \
  --http 127.0.0.1:8082
```

**Verify:**
- Bundle is extracted correctly
- Frames are replayed through the same pipeline
- Orderbook state is recreated
- Checksums are verified

---

## TUI Testing

### Test TUI in Mock Mode

```bash
# Start TUI with mock data (no internet required)
./target/release/blackbox tui \
  --symbols BTC/USD,ETH/USD,SOL/USD \
  --depth 10 \
  --mock
```

**Verify:**
- TUI starts successfully
- Shows Integrity Inspector
- Shows orderbook display
- Shows health metrics
- Press `Q` to quit

### Test TUI in Live Mode

```bash
# Start TUI with live data
./target/release/blackbox tui \
  --symbols BTC/USD,ETH/USD,SOL/USD,AVAX/USD \
  --depth 10
```

**Verify:**
- TUI connects to Kraken WebSocket
- Shows real-time orderbook data
- Shows live checksum verification
- Shows verify latency telemetry (Last/Avg/P95)
- Use `↑↓` to select symbols
- Press `R` to toggle recording
- Press `E` to export incident bundle
- Press `D` to inject fault (demo)
- Press `P` to replay last incident
- Press `?` for help
- Press `Q` to quit

### Test TUI Recording

```bash
# Start TUI with recording
./target/release/blackbox tui \
  --symbols BTC/USD \
  --depth 10 \
  --record session.ndjson

# Press R to start recording (if not started automatically)
# Wait 10-20 seconds
# Press R again to stop recording
# Press Q to quit

# Verify recording file
wc -l session.ndjson
head -3 session.ndjson
```

**Expected:**
- Recording file created
- Contains NDJSON frames
- File can be replayed

### Test TUI Replay with Fault Injection

```bash
# Replay a recording with fault injection
./target/release/blackbox tui \
  --symbols BTC/USD \
  --depth 10 \
  --replay session.ndjson \
  --fault mutate_qty \
  --once-at 50 \
  --speed 4.0
```

**Verify:**
- TUI shows replay mode
- Fault is injected at frame 50
- Checksum mismatch occurs
- Incident is captured
- Event log shows `FAULT_INJECTED`, `CHECKSUM_MISMATCH`, `INCIDENT_CAPTURED`

---

## Performance Tests

### Message Throughput

Monitor message rate:

```bash
# Watch health endpoint
watch -n 1 'curl -s http://127.0.0.1:8080/health | python3 -c "import sys, json; d=json.load(sys.stdin); s=d[\"symbols\"][0]; print(f\"Rate: {s[\"msg_rate_estimate\"]:.1f} msg/s, Total: {s[\"total_msgs\"]}\")"'
```

**Or with jq:**
```bash
watch -n 1 'curl -s http://127.0.0.1:8080/health | jq ".symbols[0] | {rate: .msg_rate_estimate, total: .total_msgs}"'
```

**Expected:**
- Message rate > 10 msg/s for active symbols
- No significant drops in rate
- Checksum success rate > 99%

### Orderbook Update Latency

Test orderbook update speed:

```bash
# Query top of book repeatedly
for i in {1..10}; do
  curl -s http://127.0.0.1:8080/book/BTC%2FUSD/top | python3 -c "import sys, json; d=json.load(sys.stdin); print(d['best_bid'][0])"
  sleep 0.1
done
```

**Expected:**
- Prices update in real-time
- No stale data (prices change over time)

### Checksum Verification Latency

Check verify latency in TUI:
1. Start TUI in live mode
2. Select a symbol with `↑↓`
3. View Integrity Inspector
4. Check "Verify Latency" section:
   - **Last**: Last verification latency in ms
   - **Avg**: Average latency
   - **P95**: 95th percentile latency

**Expected:**
- Last latency < 10ms
- Average latency < 10ms
- P95 latency < 10ms

---

## Stress Tests

### High Message Volume

```bash
# Subscribe to many symbols
./target/release/blackbox run \
  --symbols BTC/USD,ETH/USD,SOL/USD,ADA/USD,DOGE/USD,XRP/USD \
  --depth 100 \
  --http 127.0.0.1:8080
```

**Monitor:**
- Memory usage (should be stable)
- CPU usage (should be reasonable)
- No connection drops
- All symbols remain connected

### Long-Running Test

```bash
# Run for extended period
./target/release/blackbox run \
  --symbols BTC/USD \
  --depth 25 \
  --http 127.0.0.1:8080 \
  --record ./long-test.ndjson

# Let it run for 1+ hours, then check:
# - No memory leaks
# - No connection issues
# - Checksum success rate remains high
```

---

## Error Handling Tests

### Test Invalid Symbol

```bash
curl http://127.0.0.1:8080/book/INVALID%2FSYMBOL/top
```

**Expected:** `404 Not Found` or empty data with `null` values

### Test Network Disconnection

1. Start server
2. Disconnect network (or block Kraken WebSocket)
3. Wait 30 seconds
4. Check health endpoint

**Expected:**
- `"connected": false`
- Reconnection attempts logged
- Reconnects when network restored

### Test Checksum Mismatch Handling

Checksum mismatches should be:
- Logged as warnings
- Tracked in health metrics
- Trigger auto-resync (resubscribe to snapshot)

Monitor for mismatches:
```bash
curl -s http://127.0.0.1:8080/health | python3 -c "import sys, json; d=json.load(sys.stdin); s=d['symbols'][0]; print(f\"Checksum OK: {s['checksum_ok']}, Fail: {s['checksum_fail']}, Rate: {s['checksum_ok']/(s['checksum_ok']+s['checksum_fail']+1)*100:.2f}%\")"
```

---

## Browser Testing

Open endpoints in browser:
- Health: http://127.0.0.1:8080/health
- Top of Book: http://127.0.0.1:8080/book/BTC%2FUSD/top
- Full Book: http://127.0.0.1:8080/book/BTC%2FUSD?limit=5
- Root (Web UI): http://127.0.0.1:8080/

**Verify:**
- JSON renders correctly
- Data updates on refresh
- Web UI shows all symbols and health metrics
- No CORS errors (if accessing from different origin, may need CORS headers)

---

## Continuous Testing

### Run Tests in CI/CD

```yaml
# Example GitHub Actions workflow
name: Test

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - run: cargo test
      - run: cargo build --release
      - run: ./test.sh
```

---

## Troubleshooting

### Tests Fail: "Connection refused"

- Ensure server is running
- Check port is not in use: `lsof -i :8080`
- Try different port: `--http 127.0.0.1:8081`

### Tests Fail: "Empty book data"

- Wait longer (up to 30 seconds)
- Check server logs for errors
- Verify symbol name is correct (case-sensitive, use `/` not `-`)

### Tests Fail: "Checksum failures"

- Occasional failures are normal (< 1%)
- If > 1% failure rate, investigate
- Check logs for details
- Verify precision handling is correct

### Tests Fail: "No data after 30 seconds"

- Check internet connection
- Verify Kraken WebSocket is accessible
- Check firewall settings
- Review server logs for errors

### TUI Tests Fail: "Device not configured"

- Ensure you're running in a terminal (not a non-interactive shell)
- Check `$TERM` environment variable
- Try running in a different terminal emulator

---

## Test Coverage

Current test coverage includes:
- ✅ Checksum verification (Kraken example)
- ✅ Orderbook operations (snapshot, update, truncate)
- ✅ Precision formatting
- ✅ HTTP API endpoints
- ✅ WebSocket connection
- ✅ Recording/replay
- ✅ Fault injection
- ✅ Incident bundle export/replay
- ✅ TUI functionality

Areas for improvement:
- More edge cases in orderbook updates
- Reconnection scenarios
- Rate limit handling
- Error recovery
- Long-running stability tests

---

## Next Steps

1. Add property-based tests (using `proptest`)
2. Add fuzzing for frame parsing
3. Add load testing with multiple concurrent clients
4. Add integration tests in CI/CD pipeline
5. Add end-to-end tests with real Kraken WebSocket
