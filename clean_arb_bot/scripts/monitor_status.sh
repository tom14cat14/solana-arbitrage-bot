#!/bin/bash

echo "=========================================="
echo "🤖 ARB BOT LIVE TRADING STATUS"
echo "=========================================="
echo ""

# Check if bot is running
if ps aux | grep -q "[c]lean_arb_bot"; then
    echo "✅ Bot Status: RUNNING"
    PID=$(ps aux | grep "[c]lean_arb_bot" | awk '{print $2}')
    echo "   PID: $PID"
else
    echo "❌ Bot Status: NOT RUNNING"
fi
echo ""

# Check ShredStream service
if curl -s http://localhost:8080/health > /dev/null 2>&1; then
    echo "✅ ShredStream: CONNECTED"
    PRICES=$(curl -s http://localhost:8080/prices | grep -o '"total_tokens":[0-9]*' | cut -d: -f2)
    echo "   Cached prices: $PRICES"
else
    echo "❌ ShredStream: DISCONNECTED"
fi
echo ""

# Show recent activity
echo "📊 RECENT ACTIVITY (Last 2 minutes):"
echo "────────────────────────────────"

# Count opportunities
OPPORTUNITIES=$(grep -c "Triangle opportunity:" live_trading.log 2>/dev/null || echo "0")
echo "🎯 Opportunities found: $OPPORTUNITIES"

# Count errors
ERRORS=$(grep -c "ERROR" live_trading.log 2>/dev/null || echo "0")
echo "❌ Errors: $ERRORS"

# Show profits
echo ""
echo "💰 RECENT PROFITS:"
grep "Expected profit:" live_trading.log 2>/dev/null | tail -5

echo ""
echo "🚨 RECENT ERRORS:"
grep "ERROR" live_trading.log 2>/dev/null | tail -3

echo ""
echo "=========================================="
echo "📝 Full log: tail -f live_trading.log"
echo "🛑 Stop bot: pkill -f clean_arb_bot"
echo "=========================================="