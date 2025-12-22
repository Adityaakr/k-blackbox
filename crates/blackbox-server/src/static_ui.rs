pub const UI_HTML: &str = r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Kraken Blackbox Monitor</title>
    <style>
        * {
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
            background: linear-gradient(135deg, #f5f7fa 0%, #c3cfe2 100%);
            padding: 20px;
            color: #333;
        }
        .container {
            max-width: 1400px;
            margin: 0 auto;
        }
        h1 {
            text-align: center;
            color: #2c3e50;
            margin-bottom: 30px;
            font-size: 2.5em;
        }
        .status-grid {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
            gap: 20px;
            margin-bottom: 30px;
        }
        .symbol-card {
            background: white;
            border-radius: 12px;
            padding: 20px;
            box-shadow: 0 4px 6px rgba(0,0,0,0.1);
            transition: transform 0.2s;
        }
        .symbol-card:hover {
            transform: translateY(-2px);
            box-shadow: 0 6px 12px rgba(0,0,0,0.15);
        }
        .symbol-header {
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin-bottom: 15px;
        }
        .symbol-name {
            font-size: 1.5em;
            font-weight: bold;
            color: #2c3e50;
        }
        .status-badge {
            padding: 5px 15px;
            border-radius: 20px;
            font-size: 0.9em;
            font-weight: bold;
        }
        .status-ok {
            background: #2ecc71;
            color: white;
        }
        .status-warn {
            background: #f39c12;
            color: white;
        }
        .status-fail {
            background: #e74c3c;
            color: white;
        }
        .book-info {
            margin-top: 15px;
        }
        .book-row {
            display: flex;
            justify-content: space-between;
            padding: 8px 0;
            border-bottom: 1px solid #eee;
        }
        .book-label {
            font-weight: 600;
            color: #7f8c8d;
        }
        .book-value {
            color: #2c3e50;
            font-family: 'Courier New', monospace;
        }
        .spread {
            font-size: 1.2em;
            color: #27ae60;
            font-weight: bold;
        }
        .mismatch-alert {
            background: #fff3cd;
            border-left: 4px solid #ffc107;
            padding: 10px;
            margin-top: 10px;
            border-radius: 4px;
        }
        .mismatch-alert strong {
            color: #856404;
        }
        .stats {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
            gap: 10px;
            margin-top: 15px;
            font-size: 0.9em;
        }
        .stat-item {
            text-align: center;
            padding: 10px;
            background: #f8f9fa;
            border-radius: 6px;
        }
        .stat-value {
            font-size: 1.5em;
            font-weight: bold;
            color: #2c3e50;
        }
        .stat-label {
            color: #7f8c8d;
            font-size: 0.8em;
            margin-top: 5px;
        }
        .refresh-info {
            text-align: center;
            color: #7f8c8d;
            margin-top: 20px;
            font-size: 0.9em;
        }
    </style>
</head>
<body>
    <div class="container">
        <h1>🦑 Kraken Blackbox Monitor</h1>
        <div id="status-grid" class="status-grid"></div>
        <div class="refresh-info">Auto-refreshing every 2 seconds</div>
    </div>
    <script>
        async function fetchHealth() {
            try {
                const response = await fetch('/health');
                const data = await response.json();
                renderStatus(data);
            } catch (error) {
                console.error('Failed to fetch health:', error);
            }
        }
        
        async function fetchTopOfBook(symbol) {
            try {
                const response = await fetch(`/book/${symbol}/top`);
                const data = await response.json();
                return data;
            } catch (error) {
                console.error(`Failed to fetch top of book for ${symbol}:`, error);
                return null;
            }
        }
        
        function getStatusClass(status) {
            switch(status) {
                case 'OK': return 'status-ok';
                case 'WARN': return 'status-warn';
                case 'FAIL': return 'status-fail';
                default: return 'status-warn';
            }
        }
        
        function formatNumber(num) {
            if (!num) return 'N/A';
            const n = parseFloat(num);
            return n.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 8 });
        }
        
        async function renderStatus(health) {
            const grid = document.getElementById('status-grid');
            grid.innerHTML = '';
            
            for (const symbol of health.symbols) {
                const top = await fetchTopOfBook(symbol.symbol);
                
                const card = document.createElement('div');
                card.className = 'symbol-card';
                
                const mismatchAlert = symbol.last_checksum_mismatch 
                    ? `<div class="mismatch-alert">
                         <strong>⚠ Checksum Mismatch</strong><br>
                         Last: ${new Date(symbol.last_checksum_mismatch).toLocaleString()}<br>
                         Consecutive fails: ${symbol.consecutive_fails}
                       </div>`
                    : '';
                
                card.innerHTML = `
                    <div class="symbol-header">
                        <div class="symbol-name">${symbol.symbol}</div>
                        <div class="status-badge ${getStatusClass(symbol.status)}">${symbol.status}</div>
                    </div>
                    ${top ? `
                    <div class="book-info">
                        <div class="book-row">
                            <span class="book-label">Best Bid:</span>
                            <span class="book-value">${top.best_bid ? formatNumber(top.best_bid[0]) : 'N/A'}</span>
                        </div>
                        <div class="book-row">
                            <span class="book-label">Best Ask:</span>
                            <span class="book-value">${top.best_ask ? formatNumber(top.best_ask[0]) : 'N/A'}</span>
                        </div>
                        <div class="book-row">
                            <span class="book-label">Spread:</span>
                            <span class="book-value spread">${top.spread ? formatNumber(top.spread) : 'N/A'}</span>
                        </div>
                        <div class="book-row">
                            <span class="book-label">Mid:</span>
                            <span class="book-value">${top.mid ? formatNumber(top.mid) : 'N/A'}</span>
                        </div>
                    </div>
                    ` : ''}
                    <div class="stats">
                        <div class="stat-item">
                            <div class="stat-value">${(symbol.checksum_ok_rate * 100).toFixed(2)}%</div>
                            <div class="stat-label">Checksum OK</div>
                        </div>
                        <div class="stat-item">
                            <div class="stat-value">${symbol.total_msgs}</div>
                            <div class="stat-label">Total Messages</div>
                        </div>
                        <div class="stat-item">
                            <div class="stat-value">${symbol.checksum_fail}</div>
                            <div class="stat-label">Failures</div>
                        </div>
                        <div class="stat-item">
                            <div class="stat-value">${symbol.health_score}</div>
                            <div class="stat-label">Health Score</div>
                        </div>
                    </div>
                    ${mismatchAlert}
                `;
                
                grid.appendChild(card);
            }
        }
        
        // Initial load
        fetchHealth();
        
        // Auto-refresh every 2 seconds
        setInterval(fetchHealth, 2000);
    </script>
</body>
</html>
"#;

