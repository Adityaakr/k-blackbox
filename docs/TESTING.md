# Testing Guide

Comprehensive testing guide for Kraken Blackbox, covering unit tests, integration tests, and manual verification.

---

## Quick Start

Run the automated test script:

```bash
./test.sh
```

This script:
1. Starts the server
2. Waits for connection
3. Tests all HTTP endpoints
4. Checks for errors
5. Reports results

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
- Server startup
- WebSocket connection
- Health endpoint
- Top of book endpoint
- Full orderbook endpoint
- Error detection

**Expected output:**
```
🚀 Starting Kraken Blackbox Test...
✅ Health check passed
✅ Top of book data received
✅ Orderbook has 3 bid levels
✅ No errors found in logs
✅ All tests completed!
```

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
  --symbols BTC/USD,ETH/USD,SOL/USD \
  --depth 25 \
  --http 127.0.0.1:8080

# Test each symbol
curl http://127.0.0.1:8080/book/BTC%2FUSD/top
curl http://127.0.0.1:8080/book/ETH%2FUSD/top
curl http://127.0.0.1:8080/book/SOL%2FUSD/top
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
- Each line is valid JSON

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

### Test Bug Export

```bash
# While server is running, export bug bundle
curl -X POST http://127.0.0.1:8080/export-bug \
  -H "Content-Type: application/json" \
  -o bug-bundle.zip

# Extract and verify contents
unzip bug-bundle.zip -d bug-bundle/
ls -la bug-bundle/
cat bug-bundle/config.json
cat bug-bundle/health.json
head -5 bug-bundle/frames.ndjson
cat bug-bundle/instruments.json
```

**Expected:**
- ZIP file created successfully
- Contains all 4 files: `config.json`, `health.json`, `frames.ndjson`, `instruments.json`
- All files contain valid JSON

---

## Performance Tests

### Message Throughput

Monitor message rate:

```bash
# Watch health endpoint
watch -n 1 'curl -s http://127.0.0.1:8080/health | python3 -c "import sys, json; d=json.load(sys.stdin); s=d[\"symbols\"][0]; print(f\"Rate: {s[\"msg_rate_estimate\"]:.1f} msg/s, Total: {s[\"total_msgs\"]}\")"'
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

**Expected:** `404 Not Found` or empty data

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
curl -s http://127.0.0.1:8080/health | python3 -c "import sys, json; d=json.load(sys.stdin); s=d['symbols'][0]; print(f\"Checksum OK: {s['checksum_ok']}, Fail: {s['checksum_fail']}, Rate: {s['checksum_ok']/(s['checksum_ok']+s['checksum_fail'])*100:.2f}%\")"
```

---

## Browser Testing

Open endpoints in browser:

- Health: http://127.0.0.1:8080/health
- Top of Book: http://127.0.0.1:8080/book/BTC%2FUSD/top
- Full Book: http://127.0.0.1:8080/book/BTC%2FUSD?limit=5

**Verify:**
- JSON renders correctly
- Data updates on refresh
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
      - uses: actions/checkout@v2
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

- Occasional failures are normal
- If > 1% failure rate, investigate
- Check logs for details
- Verify precision handling is correct

### Tests Fail: "No data after 30 seconds"

- Check internet connection
- Verify Kraken WebSocket is accessible
- Check firewall settings
- Review server logs for errors

---

## Test Coverage

Current test coverage includes:
- ✅ Checksum verification (Kraken example)
- ✅ Orderbook operations (snapshot, update, truncate)
- ✅ Precision formatting
- ✅ HTTP API endpoints
- ✅ WebSocket connection
- ✅ Recording/replay

Areas for improvement:
- More edge cases in orderbook updates
- Reconnection scenarios
- Rate limit handling
- Error recovery

---

## Next Steps

1. Add property-based tests (using `proptest`)
2. Add fuzzing for frame parsing
3. Add load testing with multiple concurrent clients
4. Add integration tests in CI/CD pipeline

