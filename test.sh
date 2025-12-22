#!/bin/bash

# Quick test script for Kraken Blackbox
# Usage: ./test.sh

set -e

echo "🚀 Starting Kraken Blackbox Test..."
echo ""

# Kill any existing instance
pkill -f "blackbox run" 2>/dev/null || true
sleep 1

# Start the server
echo "Starting server..."
./target/release/blackbox run \
  --symbols BTC/USD \
  --depth 10 \
  --http 127.0.0.1:8080 \
  > /tmp/blackbox-test.log 2>&1 &
PID=$!
echo "Server started (PID: $PID)"
echo ""

# Wait for connection
echo "⏳ Waiting 15 seconds for WebSocket connection..."
sleep 15
echo ""

# Test 1: Health endpoint
echo "📊 Test 1: Health Endpoint"
echo "------------------------"
HEALTH=$(curl -s http://127.0.0.1:8080/health)
echo "$HEALTH" | python3 -m json.tool
STATUS=$(echo "$HEALTH" | python3 -c "import sys, json; print(json.load(sys.stdin)['status'])" 2>/dev/null || echo "ERROR")
if [ "$STATUS" = "OK" ]; then
    echo "✅ Health check passed"
else
    echo "❌ Health check failed"
fi
echo ""

# Test 2: Top of book
echo "📈 Test 2: Top of Book"
echo "---------------------"
TOP=$(curl -s http://127.0.0.1:8080/book/BTC%2FUSD/top)
echo "$TOP" | python3 -m json.tool
BID=$(echo "$TOP" | python3 -c "import sys, json; d=json.load(sys.stdin); print(d.get('best_bid', [None])[0])" 2>/dev/null || echo "")
if [ -n "$BID" ] && [ "$BID" != "None" ]; then
    echo "✅ Top of book data received"
else
    echo "❌ Top of book data missing"
fi
echo ""

# Test 3: Full book
echo "📖 Test 3: Full Orderbook (limit=3)"
echo "-----------------------------------"
BOOK=$(curl -s "http://127.0.0.1:8080/book/BTC%2FUSD?limit=3")
echo "$BOOK" | python3 -m json.tool | head -20
BIDS_COUNT=$(echo "$BOOK" | python3 -c "import sys, json; d=json.load(sys.stdin); print(len(d.get('bids', [])))" 2>/dev/null || echo "0")
if [ "$BIDS_COUNT" -gt 0 ]; then
    echo "✅ Orderbook has $BIDS_COUNT bid levels"
else
    echo "❌ Orderbook is empty"
fi
echo ""

# Test 4: Check for errors
echo "🔍 Test 4: Error Check"
echo "---------------------"
ERRORS=$(grep -i "error\|Failed to parse" /tmp/blackbox-test.log | wc -l | tr -d ' ')
if [ "$ERRORS" -eq 0 ]; then
    echo "✅ No errors found in logs"
else
    echo "⚠️  Found $ERRORS error(s) in logs:"
    grep -i "error\|Failed to parse" /tmp/blackbox-test.log | head -3
fi
echo ""

# Test 5: Metrics
echo "📊 Test 5: Metrics Endpoint"
echo "--------------------------"
METRICS=$(curl -s http://127.0.0.1:8080/metrics)
if [ -n "$METRICS" ]; then
    echo "✅ Metrics endpoint responding"
    echo "$METRICS" | head -5
else
    echo "❌ Metrics endpoint not responding"
fi
echo ""

# Summary
echo "📋 Test Summary"
echo "==============="
echo "Server PID: $PID"
echo "Log file: /tmp/blackbox-test.log"
echo ""
echo "To stop the server: kill $PID"
echo "To view logs: tail -f /tmp/blackbox-test.log"
echo ""
echo "✅ All tests completed!"

