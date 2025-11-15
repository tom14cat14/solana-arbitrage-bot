#!/bin/bash

echo "============================================"
echo "🤖 ARB BOT TRADING SUMMARY"
echo "============================================"

# Count different types of events
TOTAL_OPPS=$(grep -c "Triangle opportunity:" live_trading.log 2>/dev/null || echo "0")
TOTAL_EXECUTIONS=$(grep -c "Executing triangle opportunity" live_trading.log 2>/dev/null || echo "0")
BUNDLE_QUEUED=$(grep -c "Bundle queued:" live_trading.log 2>/dev/null || echo "0")
BUNDLE_SUBMITTED=$(grep -c "Submitting SECURE Jito bundle" live_trading.log 2>/dev/null || echo "0")

# Count errors
RATE_LIMITS=$(grep -c "429.*Rate Limit\|429 Too Many Requests" live_trading.log 2>/dev/null || echo "0")
BAD_REQUEST=$(grep -c "400 Bad Request" live_trading.log 2>/dev/null || echo "0")
TOTAL_ERRORS=$(grep -c "ERROR" live_trading.log 2>/dev/null || echo "0")

# Calculate expected profit
TOTAL_PROFIT=$(grep "Expected profit:" live_trading.log 2>/dev/null | grep -oE "[0-9]+\.[0-9]+" | awk '{sum+=$1} END {printf "%.4f", sum}')

echo ""
echo "📊 OPPORTUNITY METRICS:"
echo "   Total opportunities found: $TOTAL_OPPS"
echo "   Executed opportunities: $TOTAL_EXECUTIONS"
echo "   Bundle submissions: $BUNDLE_SUBMITTED"
echo "   Bundles queued: $BUNDLE_QUEUED"
echo ""
echo "💰 PROFIT POTENTIAL:"
echo "   Expected total profit: $TOTAL_PROFIT SOL"
if [ "$TOTAL_EXECUTIONS" -gt 0 ]; then
    AVG_PROFIT=$(echo "scale=4; $TOTAL_PROFIT / $TOTAL_EXECUTIONS" | bc)
    echo "   Average profit per trade: $AVG_PROFIT SOL"
fi
echo ""
echo "❌ ERRORS:"
echo "   429 Rate limits: $RATE_LIMITS (network congestion)"
echo "   400 Bad requests: $BAD_REQUEST (format issues)"
echo "   Total errors: $TOTAL_ERRORS"
echo ""

# Show recent activity
echo "🕐 LAST 5 OPPORTUNITIES:"
grep "Triangle opportunity:" live_trading.log 2>/dev/null | tail -5

echo ""
echo "============================================"