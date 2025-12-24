# HTTP API Reference

Kraken Blackbox exposes a local HTTP API for querying orderbooks, health metrics, and exporting incident bundles. All endpoints return JSON unless otherwise specified.

**Base URL**: `http://127.0.0.1:8080` (default, configurable via `--http` flag)

---

## Endpoints

### `GET /health`

Returns overall system health and per-symbol metrics.

**Request:**
```bash
curl http://127.0.0.1:8080/health
```

**Response:**
```json
{
  "status": "OK",
  "uptime_seconds": 3600,
  "symbols": [
    {
      "symbol": "BTC/USD",
      "connected": true,
      "last_msg_ts": "2024-01-15T10:30:45.123Z",
      "total_msgs": 125000,
      "checksum_ok": 124995,
      "checksum_fail": 5,
      "last_checksum_mismatch": "2024-01-15T10:25:12.456Z",
      "consecutive_fails": 0,
      "reconnect_count": 2,
      "msg_rate_estimate": 34.7
    }
  ]
}
```

**Response Fields:**
- `status`: Overall health status (`OK`, `WARN`, `FAIL`)
- `uptime_seconds`: Server uptime in seconds
- `symbols`: Array of per-symbol health metrics
  - `symbol`: Trading pair symbol (e.g., "BTC/USD")
  - `connected`: Whether WebSocket is connected
  - `last_msg_ts`: ISO 8601 timestamp of last message received
  - `total_msgs`: Total messages processed for this symbol
  - `checksum_ok`: Number of successful checksum verifications
  - `checksum_fail`: Number of checksum mismatches
  - `last_checksum_mismatch`: ISO 8601 timestamp of last mismatch (if any)
  - `consecutive_fails`: Number of consecutive checksum failures
  - `reconnect_count`: Number of reconnections for this symbol
  - `msg_rate_estimate`: Estimated messages per second

**Status Codes:**
- `200 OK`: Success

---

### `GET /book/:symbol/top`

Returns top-of-book data (best bid, best ask, spread, mid price).

**Request:**
```bash
curl http://127.0.0.1:8080/book/BTC%2FUSD/top
```

**Note**: URL-encode the symbol. `BTC/USD` becomes `BTC%2FUSD`.

**Response:**
```json
{
  "symbol": "BTC/USD",
  "best_bid": ["89913.3", "0.00366279"],
  "best_ask": ["89913.4", "3.56256894"],
  "spread": "0.1",
  "mid": "89913.350"
}
```

**Response Fields:**
- `symbol`: Trading pair symbol
- `best_bid`: `[price, quantity]` tuple for best bid (highest buy price), or `null` if no data
- `best_ask`: `[price, quantity]` tuple for best ask (lowest sell price), or `null` if no data
- `spread`: Spread between best bid and ask (as string), or `null` if no data
- `mid`: Mid price (average of best bid and ask, as string), or `null` if no data

**Status Codes:**
- `200 OK`: Success (may return `null` values if symbol not found)
- `404 Not Found`: Symbol not found or no data available

**Example with different symbol:**
```bash
curl http://127.0.0.1:8080/book/ETH%2FUSD/top
```

---

### `GET /book/:symbol`

Returns full orderbook (or limited depth).

**Request:**
```bash
# Full orderbook
curl http://127.0.0.1:8080/book/BTC%2FUSD

# Limited to top 5 levels
curl "http://127.0.0.1:8080/book/BTC%2FUSD?limit=5"
```

**Query Parameters:**
- `limit` (optional): Maximum number of levels to return per side (bids/asks). If omitted, returns all levels up to subscribed depth.

**Response:**
```json
{
  "symbol": "BTC/USD",
  "bids": [
    ["89913.3", "0.00366279"],
    ["89910.0", "0.009"],
    ["89909.7", "0.000051"]
  ],
  "asks": [
    ["89913.4", "3.56256894"],
    ["89913.5", "1.2"],
    ["89914.0", "0.5"]
  ]
}
```

**Response Fields:**
- `symbol`: Trading pair symbol
- `bids`: Array of `[price, quantity]` tuples, sorted descending by price (highest first)
- `asks`: Array of `[price, quantity]` tuples, sorted ascending by price (lowest first)

**Status Codes:**
- `200 OK`: Success
- `404 Not Found`: Symbol not found or no data available

**Notes:**
- Prices and quantities are returned as strings to preserve precision
- Bids are sorted highest to lowest (best bid first)
- Asks are sorted lowest to highest (best ask first)
- The number of levels returned is limited by the `limit` parameter or the subscribed depth

---

### `GET /metrics`

Returns Prometheus-formatted metrics.

**Request:**
```bash
curl http://127.0.0.1:8080/metrics
```

**Response:**
```
# Prometheus metrics endpoint
# Install metrics exporter in main.rs
```

**Status Codes:**
- `200 OK`: Success

**Note**: This endpoint is a placeholder. Full Prometheus metrics integration is planned for future releases.

---

### `POST /export-bug`

Exports an incident bundle ZIP file containing configuration, health state, recent WebSocket frames, orderbook snapshot, and instrument information.

**Request:**
```bash
curl -X POST http://127.0.0.1:8080/export-bug -o incident.zip
```

**Response:**
- **Content-Type**: `application/zip`
- **Content-Disposition**: `attachment; filename="incident_<timestamp>_<reason>.zip"`
- **Body**: ZIP file bytes

**Status Codes:**
- `200 OK`: Success, ZIP file returned
- `500 Internal Server Error`: Failed to create incident bundle

**Incident Bundle Contents:**
The ZIP file contains:
- `metadata.json`: Incident metadata (incident info, config, health, instrument, book_top)
- `config.json`: Configuration snapshot (symbols, timestamp)
- `health.json`: Current health state (same as `/health` endpoint)
- `frames.ndjson`: Raw WebSocket frames from last 30 seconds before incident to 5 seconds after (NDJSON format, one `RecordedFrame` per line)
- `instrument.json` (optional): Instrument snapshot with precisions and increments
- `book_top.json` (optional): Top of book snapshot at incident time

**Example:**
```bash
# Export incident bundle
curl -X POST http://127.0.0.1:8080/export-bug -o incident.zip

# Extract and inspect
unzip incident.zip -d incident/
ls -la incident/

# View contents
cat incident/metadata.json | jq .
cat incident/health.json | jq .
head -5 incident/frames.ndjson
```

**Note**: The ZIP file is also saved to `./incidents/<incident_id>.zip` on the server.

---

## Error Responses

All endpoints may return error responses in the following format:

**Status Code:** `4xx` or `5xx`

**Response:**
```json
{
  "error": "Error message description"
}
```

**Common Errors:**
- `404 Not Found`: Symbol not found or endpoint doesn't exist
- `500 Internal Server Error`: Server error (check logs)

---

## Rate Limiting

Currently, there are no rate limits on the HTTP API. However, for production use, consider:
- Implementing rate limiting per IP
- Adding authentication if exposing publicly
- Using a reverse proxy (nginx, Caddy) for rate limiting

---

## CORS

The HTTP API does not set CORS headers by default. For browser-based clients, you may need to:
- Add CORS middleware (tower-http CORS layer)
- Use a reverse proxy to add CORS headers
- Access from same origin only

---

## Examples

### Monitor health continuously
```bash
watch -n 1 'curl -s http://127.0.0.1:8080/health | python3 -m json.tool'
```

### Get top of book for multiple symbols
```bash
for symbol in BTC/USD ETH/USD SOL/USD; do
  echo "=== $symbol ==="
  curl -s "http://127.0.0.1:8080/book/$(echo $symbol | sed 's/\//%2F/')/top" | python3 -m json.tool
  echo ""
done
```

### Export incident bundle and extract
```bash
curl -X POST http://127.0.0.1:8080/export-bug -o incident.zip && \
unzip -q incident.zip -d incident/ && \
ls -lh incident/
```

---

## SDK Usage

The HTTP API is provided by the example application (`blackbox-server`). The SDK itself (`blackbox-core` + `blackbox-ws`) is event-driven and doesn't require HTTP. See the main README for SDK usage examples.
