#!/bin/bash
# Monitor paper trading performance

LOG_FILE="paper_trading_test.log"

echo "======================================"
echo "📊 Paper Trading Performance Monitor"
echo "======================================"
echo ""

# Total trades
TOTAL_TRADES=$(grep -c "Paper profit:" $LOG_FILE 2>/dev/null || echo "0")
echo "Total Trades: $TOTAL_TRADES"

# Current total profit
TOTAL_PROFIT=$(grep "Total:" $LOG_FILE | tail -1 | grep -oP '\(Total: \K[0-9.]+' || echo "0")
echo "Total Profit: ${TOTAL_PROFIT} SOL"

# Average profit per trade
if [ "$TOTAL_TRADES" -gt 0 ]; then
    AVG_PROFIT=$(echo "scale=4; $TOTAL_PROFIT / $TOTAL_TRADES" | bc)
    echo "Avg per Trade: ${AVG_PROFIT} SOL"
fi

echo ""
echo "======================================"
echo "💰 Last 10 Trades:"
echo "======================================"
grep "Paper profit:" $LOG_FILE | tail -10 | while read line; do
    PROFIT=$(echo "$line" | grep -oP 'Paper profit: \K[0-9.]+')
    TOTAL=$(echo "$line" | grep -oP '\(Total: \K[0-9.]+')
    echo "  +${PROFIT} SOL (Running total: ${TOTAL} SOL)"
done

echo ""
echo "======================================"
echo "⚠️  Rejections (Recent):"
echo "======================================"
grep "Rejecting unrealistic spread" $LOG_FILE | tail -5 | while read line; do
    SPREAD=$(echo "$line" | grep -oP 'spread: \K[0-9.]+')
    TOKEN=$(echo "$line" | grep -oP 'for \K[A-Za-z0-9]+')
    echo "  Rejected: ${TOKEN} (${SPREAD}% spread - too high)"
done

echo ""
echo "======================================"
echo "📈 Profitability Check:"
echo "======================================"
# Check if profitable after costs
grep "Cost validation passed" $LOG_FILE | tail -3 | while read line; do
    echo "  ✅ Cost validation: PASSED"
done

echo ""
echo "======================================"
echo "💡 Watching for new trades..."
echo "======================================"
echo "Press Ctrl+C to stop monitoring"
echo ""

# Live tail of new profits
tail -f $LOG_FILE | grep --line-buffered "Paper profit:" | while read line; do
    PROFIT=$(echo "$line" | grep -oP 'Paper profit: \K[0-9.]+')
    TOTAL=$(echo "$line" | grep -oP '\(Total: \K[0-9.]+')
    TIMESTAMP=$(date "+%H:%M:%S")
    echo "[$TIMESTAMP] +${PROFIT} SOL → Total: ${TOTAL} SOL"
done
