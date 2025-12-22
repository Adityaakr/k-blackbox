# Quick Test Guide

Fastest way to verify Kraken Blackbox is working correctly.

---

## 🚀 Automated Test (Recommended)

Run the test script:

```bash
./test.sh
```

**What it does:**
1. Starts the server
2. Waits for WebSocket connection
3. Tests all endpoints
4. Reports results

**Expected output:**
```
🚀 Starting Kraken Blackbox Test...
✅ Health check passed
✅ Top of book data received
✅ Orderbook has 3 bid levels
✅ No errors found in logs
✅ All tests completed!
```

**Time:** ~20 seconds

---

## 📝 Manual Quick Test (3 Steps)

### Step 1: Start Server

```bash
./target/release/blackbox run \
  --symbols BTC/USD \
  --depth 10 \
  --http 127.0.0.1:8080
```

**Wait 15-20 seconds** for connection to establish.

### Step 2: Test Health

```bash
curl http://127.0.0.1:8080/health | python3 -m json.tool
```

**✅ Success if you see:**
- `"status": "OK"`
- `"connected": true`
- `"total_msgs"` > 0
- `"checksum_ok"` > 0

### Step 3: Test Orderbook

```bash
curl http://127.0.0.1:8080/book/BTC%2FUSD/top | python3 -m json.tool
```

**✅ Success if you see:**
- Valid price/quantity pairs
- `best_bid` and `best_ask` present
- Positive spread

**Time:** ~30 seconds total

---

## 🎯 One-Liner Test

Copy and paste this entire block:

```bash
cd /Users/adityakumar/Desktop/kblackbox && \
./target/release/blackbox run --symbols BTC/USD --depth 10 --http 127.0.0.1:8080 > /tmp/bb.log 2>&1 & \
sleep 20 && \
echo "=== Health ===" && \
curl -s http://127.0.0.1:8080/health | python3 -m json.tool && \
echo "" && \
echo "=== Top of Book ===" && \
curl -s http://127.0.0.1:8080/book/BTC%2FUSD/top | python3 -m json.tool
```

This:
- Starts the server in background
- Waits 20 seconds
- Tests health endpoint
- Tests top of book endpoint
- Shows results

**Time:** ~25 seconds

---

## 🌐 Browser Test

1. Start server:
   ```bash
   ./target/release/blackbox run --symbols BTC/USD --depth 10 --http 127.0.0.1:8080
   ```

2. Wait 15 seconds

3. Open in browser:
   - Health: http://127.0.0.1:8080/health
   - Top of Book: http://127.0.0.1:8080/book/BTC%2FUSD/top

**✅ Success:** You see JSON data in browser

---

## ✅ Success Indicators

### Health Endpoint
- ✅ `"status": "OK"`
- ✅ `"connected": true`
- ✅ `"total_msgs"` increasing
- ✅ `"checksum_fail": 0` (or very low)

### Orderbook Endpoint
- ✅ Returns valid prices
- ✅ Prices update on refresh
- ✅ Bids < Asks (positive spread)

### No Errors
- ✅ No "Failed to parse" in logs
- ✅ No connection errors
- ✅ No rate limit errors

---

## 🐛 Quick Troubleshooting

### "Connection refused"
```bash
# Check if server is running
ps aux | grep blackbox

# Check if port is in use
lsof -i :8080

# Try different port
./target/release/blackbox run --symbols BTC/USD --http 127.0.0.1:8081
```

### "Empty book data"
```bash
# Wait longer (up to 30 seconds)
# Check server terminal for errors
# Verify symbol: BTC/USD (case-sensitive, use / not -)
```

### "No data after 30 seconds"
```bash
# Check internet connection
# Check server logs: tail -f /tmp/bb.log
# Try different symbol: ETH/USD
```

---

## 📊 Test Multiple Symbols

```bash
# Start with multiple symbols
./target/release/blackbox run \
  --symbols BTC/USD,ETH/USD,SOL/USD \
  --depth 25 \
  --http 127.0.0.1:8080

# Test each (wait 20 seconds first)
curl http://127.0.0.1:8080/book/BTC%2FUSD/top
curl http://127.0.0.1:8080/book/ETH%2FUSD/top
curl http://127.0.0.1:8080/book/SOL%2FUSD/top
```

---

## 🎬 Test Recording

```bash
# Start with recording
./target/release/blackbox run \
  --symbols BTC/USD \
  --depth 10 \
  --http 127.0.0.1:8080 \
  --record ./test.ndjson

# Let it run 30 seconds, then Ctrl+C

# Verify recording
wc -l test.ndjson
head -3 test.ndjson
```

**✅ Success:** File exists with NDJSON lines

---

## 🔍 Verify Checksum Verification

```bash
# Monitor checksum stats
watch -n 2 'curl -s http://127.0.0.1:8080/health | python3 -c "import sys, json; d=json.load(sys.stdin); s=d[\"symbols\"][0]; print(f\"OK: {s[\"checksum_ok\"]}, Fail: {s[\"checksum_fail\"]}, Rate: {s[\"checksum_ok\"]/(s[\"checksum_ok\"]+s[\"checksum_fail\"]+1)*100:.2f}%\")"'
```

**✅ Success:** Checksum success rate > 99%

---

## 📦 Test Bug Export

```bash
# While server is running
curl -X POST http://127.0.0.1:8080/export-bug -o bug.zip

# Verify
unzip -l bug.zip
unzip -q bug.zip -d bug/
ls -la bug/
```

**✅ Success:** ZIP contains `config.json`, `health.json`, `frames.ndjson`, `instruments.json`

---

## ⚡ Performance Check

```bash
# Monitor message rate
watch -n 1 'curl -s http://127.0.0.1:8080/health | python3 -c "import sys, json; d=json.load(sys.stdin); s=d[\"symbols\"][0]; print(f\"Rate: {s[\"msg_rate_estimate\"]:.1f} msg/s\")"'
```

**✅ Success:** Message rate > 10 msg/s for active symbols

---

## 🎯 All Tests Passed?

If all checks pass:
- ✅ Health shows OK
- ✅ Orderbook data is valid
- ✅ No errors in logs
- ✅ Checksum success rate > 99%

**You're ready to use Kraken Blackbox!** 🎉

For detailed testing, see [TESTING.md](./TESTING.md).

