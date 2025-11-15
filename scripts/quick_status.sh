#!/bin/bash
# Quick Status Check - One Line Summary
# Usage: ./quick_status.sh

LOG_FILE="logs/live_trading.log"

# Get recent stats (last 500 lines)
RECENT=$(tail -500 "$LOG_FILE")

# Count metrics
OPPS=$(echo "$RECENT" | grep -c "Triangle opportunity:")
SIMS_PASS=$(echo "$RECENT" | grep -c "executed successfully")
FINAL_FAIL=$(echo "$RECENT" | grep -c "simulation failed - skipping")
JITO_TOTAL=$(grep -c "JITO bundle submitted" "$LOG_FILE")
CRITICAL=$(grep -c "Custom(101)" "$LOG_FILE")

# Get last profit
LAST_PROFIT=$(echo "$RECENT" | grep "Net profit:" | tail -1 | grep -oP 'Net profit: \K[0-9.]+')

# Calculate success rate
if [ "$SIMS_PASS" -gt 0 ]; then
    SUCCESS_RATE=$(awk "BEGIN {printf \"%.0f\", ($SIMS_PASS - $FINAL_FAIL) / $SIMS_PASS * 100}")
else
    SUCCESS_RATE="0"
fi

# Status color
if [ "$CRITICAL" -gt 0 ]; then
    STATUS="🔴 CRITICAL ERROR"
elif [ "$JITO_TOTAL" -gt 0 ]; then
    STATUS="🟢 TRADING"
elif [ "$SUCCESS_RATE" -gt 50 ]; then
    STATUS="🟢 VERY CLOSE"
elif [ "$SUCCESS_RATE" -gt 20 ]; then
    STATUS="🟡 GETTING CLOSE"
elif [ "$SUCCESS_RATE" -gt 0 ]; then
    STATUS="🟡 FINDING OPPS"
else
    STATUS="🔴 HIGH VOLATILITY"
fi

# One-line summary
echo "[$STATUS] Opps:$OPPS | PassRate:${SUCCESS_RATE}% | JITO:$JITO_TOTAL | LastProfit:${LAST_PROFIT:-N/A} SOL | Errors(101):$CRITICAL"
