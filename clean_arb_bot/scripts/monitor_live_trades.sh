#!/bin/bash
# Real-time monitoring script for live trading
# Shows only important events

LOG_DIR="/home/tom14cat14/Arb_Bot/clean_arb_bot/logs"
LATEST_LOG=$(find "$LOG_DIR" -name "live_trading_*.log" -type f -printf '%T@ %p\n' 2>/dev/null | sort -rn | head -1 | cut -d' ' -f2-)

if [ -z "$LATEST_LOG" ]; then
    echo "No log file found. Bot not started yet?"
    exit 1
fi

echo "Monitoring: $LATEST_LOG"
echo "Press Ctrl+C to stop monitoring"
echo ""
echo "=========================================="
echo "LIVE TRADING MONITOR"
echo "=========================================="
echo ""

# Follow log and filter for important events
tail -f "$LATEST_LOG" | grep --line-buffered -E \
    "(Opportunity detected|Submitting|executed|failed|ERROR|WARN|Balance|Profit|Loss|Circuit|Stop)"
