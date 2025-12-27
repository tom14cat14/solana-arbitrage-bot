#!/bin/bash
# Check current trading status and statistics

LOG_DIR="/home/tom14cat14/Arb_Bot/clean_arb_bot/logs"
LATEST_LOG=$(find "$LOG_DIR" -name "live_trading_*.log" -type f -printf '%T@ %p\n' 2>/dev/null | sort -rn | head -1 | cut -d' ' -f2-)

echo "=========================================="
echo "LIVE TRADING STATUS CHECK"
echo "=========================================="
echo "Time: $(date)"
echo ""

# Check if bot is running
if pgrep -f "clean_arb_bot" > /dev/null; then
    echo "✅ Bot Status: RUNNING"
else
    echo "❌ Bot Status: NOT RUNNING"
fi
echo ""

# Check wallet balance
echo "Wallet Balance:"
solana balance 9WrFdecsvMogYEtwjGrCBs4LrfnZhm9QKigD4CdcD3kA 2>/dev/null || echo "Unable to check balance"
echo ""

# Check ShredStream service
echo "ShredStream Service:"
if curl -s http://localhost:8080/health > /dev/null 2>&1; then
    echo "✅ RUNNING"
else
    echo "❌ NOT RUNNING"
fi
echo ""

if [ -n "$LATEST_LOG" ]; then
    echo "Latest Log: $LATEST_LOG"
    echo ""

    # Count trades
    echo "Trade Statistics (from current session):"
    echo "  Opportunities detected: $(grep -c "Opportunity detected" "$LATEST_LOG" 2>/dev/null || echo 0)"
    echo "  Trades submitted: $(grep -c "Submitting" "$LATEST_LOG" 2>/dev/null || echo 0)"
    echo "  Successful trades: $(grep -c "executed successfully" "$LATEST_LOG" 2>/dev/null || echo 0)"
    echo "  Failed trades: $(grep -c "failed" "$LATEST_LOG" 2>/dev/null || echo 0)"
    echo "  Errors: $(grep -c "ERROR" "$LATEST_LOG" 2>/dev/null || echo 0)"
    echo "  Warnings: $(grep -c "WARN" "$LATEST_LOG" 2>/dev/null || echo 0)"
    echo ""

    # Show last 5 important events
    echo "Last 5 Important Events:"
    echo "----------------------------------------"
    grep -E "(Opportunity detected|Submitting|executed|failed|ERROR|WARN)" "$LATEST_LOG" | tail -5
    echo ""
else
    echo "No log file found."
fi

echo "=========================================="
