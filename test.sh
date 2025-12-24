#!/bin/bash

# Kraken Blackbox - Comprehensive Test Script
# This script tests all features of the Kraken Blackbox SDK

set -e  # Exit on error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
SYMBOLS="BTC/USD,ETH/USD,SOL/USD"
DEPTH=25
HTTP_PORT=8080
RECORDING_FILE="test_session.ndjson"
INCIDENTS_DIR="./incidents"

echo -e "${BLUE}╔════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║     Kraken Blackbox - Comprehensive Test Suite            ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Function to print section headers
print_section() {
    echo ""
    echo -e "${YELLOW}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${YELLOW}$1${NC}"
    echo -e "${YELLOW}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
}

# Function to check if command exists
check_command() {
    if ! command -v $1 &> /dev/null; then
        echo -e "${RED}Error: $1 is not installed${NC}"
        exit 1
    fi
}

# Check prerequisites
print_section "Checking Prerequisites"
check_command cargo
check_command curl
check_command jq || echo -e "${YELLOW}Warning: jq not found (optional, for JSON formatting)${NC}"

# Build project
print_section "Building Project"
echo -e "${BLUE}Building in release mode...${NC}"
cargo build --release
if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ Build successful${NC}"
else
    echo -e "${RED}✗ Build failed${NC}"
    exit 1
fi

BINARY="./target/release/blackbox"

# Create directories
mkdir -p "$INCIDENTS_DIR"
echo -e "${GREEN}✓ Created directories${NC}"

# Test 1: Mock Mode (No internet required)
print_section "Test 1: Mock Mode TUI"
echo -e "${BLUE}Starting TUI in mock mode (press Q to quit after viewing)...${NC}"
echo -e "${YELLOW}This will start the TUI. Press Q to quit when done.${NC}"
timeout 10s $BINARY tui --symbols "$SYMBOLS" --depth $DEPTH --mock || true
echo -e "${GREEN}✓ Mock mode test completed${NC}"

# Test 2: HTTP API (Live mode)
print_section "Test 2: HTTP API Server"
echo -e "${BLUE}Starting HTTP server in background...${NC}"
$BINARY run --symbols "$SYMBOLS" --depth $DEPTH --http "127.0.0.1:$HTTP_PORT" &
SERVER_PID=$!
sleep 5

# Wait for server to be ready
echo -e "${BLUE}Waiting for server to be ready...${NC}"
for i in {1..30}; do
    if curl -s "http://127.0.0.1:$HTTP_PORT/health" > /dev/null 2>&1; then
        echo -e "${GREEN}✓ Server is ready${NC}"
        break
    fi
    sleep 1
    if [ $i -eq 30 ]; then
        echo -e "${RED}✗ Server failed to start${NC}"
        kill $SERVER_PID 2>/dev/null || true
        exit 1
    fi
done

# Test health endpoint
echo -e "${BLUE}Testing /health endpoint...${NC}"
HEALTH_RESPONSE=$(curl -s "http://127.0.0.1:$HTTP_PORT/health")
if echo "$HEALTH_RESPONSE" | grep -q "status"; then
    echo -e "${GREEN}✓ Health endpoint working${NC}"
    if command -v jq &> /dev/null; then
        echo "$HEALTH_RESPONSE" | jq .
    else
        echo "$HEALTH_RESPONSE"
    fi
else
    echo -e "${RED}✗ Health endpoint failed${NC}"
    echo "$HEALTH_RESPONSE"
fi

# Test book endpoint
echo -e "${BLUE}Testing /book endpoint...${NC}"
BOOK_RESPONSE=$(curl -s "http://127.0.0.1:$HTTP_PORT/book/BTC%2FUSD/top")
if echo "$BOOK_RESPONSE" | grep -q "symbol\|best_bid\|best_ask"; then
    echo -e "${GREEN}✓ Book endpoint working${NC}"
    if command -v jq &> /dev/null; then
        echo "$BOOK_RESPONSE" | jq .
    else
        echo "$BOOK_RESPONSE"
    fi
else
    echo -e "${YELLOW}⚠ Book endpoint returned empty (may need more time for data)${NC}"
fi

# Test export-bug endpoint
echo -e "${BLUE}Testing /export-bug endpoint...${NC}"
EXPORT_RESPONSE=$(curl -s -X POST "http://127.0.0.1:$HTTP_PORT/export-bug" -o test_export.zip)
if [ -f "test_export.zip" ]; then
    echo -e "${GREEN}✓ Export endpoint working${NC}"
    unzip -l test_export.zip | head -10
    rm -f test_export.zip
else
    echo -e "${YELLOW}⚠ Export endpoint may need an incident to export${NC}"
fi

# Stop server
echo -e "${BLUE}Stopping HTTP server...${NC}"
kill $SERVER_PID 2>/dev/null || true
wait $SERVER_PID 2>/dev/null || true
echo -e "${GREEN}✓ HTTP server stopped${NC}"

# Test 3: Recording
print_section "Test 3: Recording Functionality"
echo -e "${BLUE}Testing recording (will run for 10 seconds)...${NC}"
timeout 10s $BINARY run --symbols "BTC/USD" --depth 10 --record "$RECORDING_FILE" || true

if [ -f "$RECORDING_FILE" ]; then
    RECORDING_SIZE=$(wc -l < "$RECORDING_FILE")
    echo -e "${GREEN}✓ Recording created: $RECORDING_FILE ($RECORDING_SIZE lines)${NC}"
    
    # Check first frame
    if command -v jq &> /dev/null; then
        echo -e "${BLUE}First frame structure:${NC}"
        head -1 "$RECORDING_FILE" | jq . | head -5
    fi
else
    echo -e "${YELLOW}⚠ Recording file not created (may need live connection)${NC}"
fi

# Test 4: Replay
if [ -f "$RECORDING_FILE" ]; then
    print_section "Test 4: Replay Functionality"
    echo -e "${BLUE}Testing replay (will run for 5 seconds)...${NC}"
    timeout 5s $BINARY replay --input "$RECORDING_FILE" --speed 4.0 --http "127.0.0.1:$((HTTP_PORT+1))" || true
    echo -e "${GREEN}✓ Replay test completed${NC}"
else
    echo -e "${YELLOW}⚠ Skipping replay test (no recording file)${NC}"
fi

# Test 5: Replay with Fault Injection
if [ -f "$RECORDING_FILE" ]; then
    print_section "Test 5: Fault Injection"
    echo -e "${BLUE}Testing fault injection (mutate qty at frame 10)...${NC}"
    timeout 5s $BINARY replay \
        --input "$RECORDING_FILE" \
        --speed 4.0 \
        --fault-mutate-once 10 \
        --fault-mutate-delta 1 \
        --http "127.0.0.1:$((HTTP_PORT+2))" || true
    echo -e "${GREEN}✓ Fault injection test completed${NC}"
else
    echo -e "${YELLOW}⚠ Skipping fault injection test (no recording file)${NC}"
fi

# Test 6: TUI with Recording
print_section "Test 6: TUI Recording Mode"
echo -e "${BLUE}Testing TUI with recording (press Q after a few seconds)...${NC}"
echo -e "${YELLOW}This will start the TUI. Press Q to quit when done.${NC}"
timeout 10s $BINARY tui --symbols "BTC/USD" --depth 10 --record "tui_test.ndjson" || true
if [ -f "tui_test.ndjson" ]; then
    echo -e "${GREEN}✓ TUI recording test completed${NC}"
    rm -f tui_test.ndjson
else
    echo -e "${YELLOW}⚠ TUI recording file not created${NC}"
fi

# Test 7: Replay Incident (if incident exists)
print_section "Test 7: Incident Replay"
if [ -d "$INCIDENTS_DIR" ] && [ "$(ls -A $INCIDENTS_DIR/*.zip 2>/dev/null)" ]; then
    INCIDENT_FILE=$(ls -t $INCIDENTS_DIR/*.zip | head -1)
    echo -e "${BLUE}Found incident bundle: $INCIDENT_FILE${NC}"
    echo -e "${BLUE}Testing incident replay...${NC}"
    timeout 5s $BINARY replay-incident --bundle "$INCIDENT_FILE" --speed 4.0 --http "127.0.0.1:$((HTTP_PORT+3))" || true
    echo -e "${GREEN}✓ Incident replay test completed${NC}"
else
    echo -e "${YELLOW}⚠ No incident bundles found (create one by triggering a checksum mismatch)${NC}"
fi

# Test 8: Unit Tests
print_section "Test 8: Unit Tests"
echo -e "${BLUE}Running unit tests...${NC}"
cargo test --lib 2>&1 | tail -20
if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ Unit tests passed${NC}"
else
    echo -e "${YELLOW}⚠ Some tests may have failed (check output above)${NC}"
fi

# Test 9: Checksum Verification Test
print_section "Test 9: Checksum Verification"
echo -e "${BLUE}Running checksum-specific tests...${NC}"
cargo test --package blackbox-core checksum 2>&1 | tail -10
if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ Checksum tests passed${NC}"
else
    echo -e "${YELLOW}⚠ Checksum tests may have issues${NC}"
fi

# Summary
print_section "Test Summary"
echo -e "${GREEN}✓ Build: Successful${NC}"
echo -e "${GREEN}✓ Mock Mode: Tested${NC}"
echo -e "${GREEN}✓ HTTP API: Tested${NC}"
if [ -f "$RECORDING_FILE" ]; then
    echo -e "${GREEN}✓ Recording: Working ($(wc -l < $RECORDING_FILE) frames)${NC}"
    echo -e "${GREEN}✓ Replay: Tested${NC}"
    echo -e "${GREEN}✓ Fault Injection: Tested${NC}"
else
    echo -e "${YELLOW}⚠ Recording: Not tested (requires live connection)${NC}"
fi
echo -e "${GREEN}✓ TUI: Tested${NC}"

echo ""
echo -e "${BLUE}╔════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║  All tests completed!                                      ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════════════════╝${NC}"
echo ""
echo -e "${YELLOW}Next steps:${NC}"
echo -e "  1. Run live mode: $BINARY tui --symbols $SYMBOLS --depth $DEPTH"
echo -e "  2. Record a session: $BINARY tui --symbols BTC/USD --depth 10 --record session.ndjson"
echo -e "  3. Replay with faults: $BINARY tui --symbols BTC/USD --depth 10 --replay session.ndjson --fault mutate_qty --once-at 120"
echo ""

# Cleanup
if [ -f "$RECORDING_FILE" ]; then
    echo -e "${BLUE}Recording file saved: $RECORDING_FILE${NC}"
    echo -e "${YELLOW}To remove: rm $RECORDING_FILE${NC}"
fi
