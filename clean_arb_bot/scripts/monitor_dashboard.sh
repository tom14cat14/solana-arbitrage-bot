#!/bin/bash
# Live Monitoring Dashboard for Arbitrage Bot
# Reads logs without affecting bot performance
# Usage: ./monitor_dashboard.sh

LOG_FILE="../logs/arb_bot.log"
REFRESH_INTERVAL=2  # seconds

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Function to update screen - no clearing, just repositions cursor
clear_screen() {
    # Just move cursor to top-left without clearing
    # Content will be overwritten in place
    printf '\033[H'
}

# Function to get recent stats
get_stats() {
    # Time window: last 60 seconds of logs
    RECENT_LOGS=$(tail -1000 "$LOG_FILE")

    # Count opportunities in recent window (sanitize to ensure single integer)
    OPPS_DETECTED=$(echo "$RECENT_LOGS" | grep -c "Triangle opportunity:" 2>/dev/null | head -1 | tr -d '\n' || echo "0")
    OPPS_DETECTED=${OPPS_DETECTED:-0}

    # Count successful initial simulations (sanitize to ensure single integer)
    SIMS_PASSED=$(echo "$RECENT_LOGS" | grep -c "executed successfully" 2>/dev/null | head -1 | tr -d '\n' || echo "0")
    SIMS_PASSED=${SIMS_PASSED:-0}

    # Count final simulation failures (sanitize to ensure single integer)
    FINAL_SIM_FAILED=$(echo "$RECENT_LOGS" | grep -c "simulation failed - skipping JITO" 2>/dev/null | head -1 | tr -d '\n' || echo "0")
    FINAL_SIM_FAILED=${FINAL_SIM_FAILED:-0}

    # Count JITO submissions (sanitize to ensure single integer)
    JITO_SUBMITTED=$(grep -c "JITO bundle submitted" "$LOG_FILE" 2>/dev/null | head -1 | tr -d '\n' || echo "0")
    JITO_SUBMITTED=${JITO_SUBMITTED:-0}

    # Count Custom(101) errors - CRITICAL (sanitize to ensure single integer)
    CRITICAL_ERRORS=$(grep -c "Custom(101)" "$LOG_FILE" 2>/dev/null | head -1 | tr -d '\n' || echo "0")
    CRITICAL_ERRORS=${CRITICAL_ERRORS:-0}

    # Count Custom(3007) errors - market volatility (sanitize to ensure single integer)
    MARKET_ERRORS=$(echo "$RECENT_LOGS" | grep -c "Custom(3007)" 2>/dev/null | head -1 | tr -d '\n' || echo "0")
    MARKET_ERRORS=${MARKET_ERRORS:-0}

    # Get last 5 profitable opportunities
    LAST_PROFITS=$(echo "$RECENT_LOGS" | grep "Net profit:" | tail -5)

    # Get last opportunity details
    LAST_OPP=$(echo "$RECENT_LOGS" | grep "Triangle opportunity:" | tail -1)

    # Calculate how close to profitable execution
    # This is the key metric you asked for!
    if [ "$SIMS_PASSED" -gt 0 ]; then
        CLOSE_RATIO=$(awk "BEGIN {printf \"%.1f\", ($SIMS_PASSED - $FINAL_SIM_FAILED) / $SIMS_PASSED * 100}")
    else
        CLOSE_RATIO="0.0"
    fi
}

# Main display loop
while true; do
    clear_screen
    get_stats

    echo -e "${CYAN}═══════════════════════════════════════════════════════════════${NC}"
    echo -e "${CYAN}         ARBITRAGE BOT - LIVE MONITORING DASHBOARD${NC}"
    echo -e "${CYAN}═══════════════════════════════════════════════════════════════${NC}"
    echo ""
    echo -e "${BLUE}📊 Last Updated:${NC} $(date '+%Y-%m-%d %H:%M:%S')"
    echo -e "${BLUE}📂 Log File:${NC} $LOG_FILE"
    echo ""

    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${CYAN}  CRITICAL HEALTH CHECK${NC}"
    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

    if [ "$CRITICAL_ERRORS" -eq 0 ]; then
        echo -e "${GREEN}✅ Custom(101) Errors (Code Bugs):${NC} $CRITICAL_ERRORS ${GREEN}(PERFECT)${NC}"
    else
        echo -e "${RED}❌ Custom(101) Errors (Code Bugs):${NC} $CRITICAL_ERRORS ${RED}(STOP BOT!)${NC}"
    fi

    echo ""

    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${CYAN}  OPPORTUNITY PIPELINE (Recent Activity)${NC}"
    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

    echo -e "${YELLOW}🔍 Opportunities Detected:${NC} $OPPS_DETECTED"
    echo -e "${GREEN}✅ Initial Simulations Passed:${NC} $SIMS_PASSED"
    echo -e "${YELLOW}⏳ Final Simulations Failed:${NC} $FINAL_SIM_FAILED ${BLUE}(market volatility)${NC}"
    echo -e "${GREEN}🚀 JITO Bundles Submitted:${NC} $JITO_SUBMITTED ${GREEN}(TOTAL ALL TIME)${NC}"
    echo ""

    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${CYAN}  🎯 HOW CLOSE TO EXECUTION? (Your Key Metric)${NC}"
    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

    # This is what you asked for - how close are we to profit?
    echo -e "${YELLOW}📊 Execution Success Rate:${NC} $CLOSE_RATIO%"
    echo ""

    # Visual indicator
    if [ "$JITO_SUBMITTED" -gt 0 ]; then
        echo -e "${GREEN}🎉 STATUS: TRADES ARE LANDING! Check wallet!${NC}"
    elif (( $(echo "$CLOSE_RATIO > 50" | bc -l) )); then
        echo -e "${GREEN}🟢 STATUS: Very close! Market conditions improving${NC}"
    elif (( $(echo "$CLOSE_RATIO > 20" | bc -l) )); then
        echo -e "${YELLOW}🟡 STATUS: Getting closer, some stable windows${NC}"
    elif (( $(echo "$CLOSE_RATIO > 0" | bc -l) )); then
        echo -e "${YELLOW}🟠 STATUS: Finding opportunities, waiting for stability${NC}"
    else
        echo -e "${RED}🔴 STATUS: High volatility, opportunities going stale quickly${NC}"
    fi

    echo ""
    echo -e "${BLUE}📈 What this means:${NC}"
    echo -e "   • ${SIMS_PASSED} opportunities passed initial checks"
    echo -e "   • ${FINAL_SIM_FAILED} became stale before final execution"
    echo -e "   • Need ${BLUE}market window > 40-50ms${NC} for trade to land"
    echo ""

    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${CYAN}  💰 RECENT PROFITABLE OPPORTUNITIES${NC}"
    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

    if [ -n "$LAST_PROFITS" ]; then
        echo "$LAST_PROFITS" | while read -r line; do
            # Extract profit and retention
            profit=$(echo "$line" | grep -oP 'Net profit: \K[0-9.]+')
            retention=$(echo "$line" | grep -oP '\(\K[0-9.]+')

            if [ -n "$profit" ]; then
                echo -e "${GREEN}💵${NC} Net Profit: ${GREEN}${profit} SOL${NC} (${retention}% retention)"
            fi
        done
    else
        echo -e "${YELLOW}⏳ Waiting for profitable opportunities...${NC}"
    fi

    echo ""

    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${CYAN}  🔥 LATEST OPPORTUNITY DETECTED${NC}"
    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

    if [ -n "$LAST_OPP" ]; then
        echo "$LAST_OPP" | sed 's/\[2m.*\[0m//g; s/\[32m//g; s/\[33m//g; s/\[34m//g; s/\[0m//g'
    else
        echo -e "${YELLOW}⏳ No recent opportunities${NC}"
    fi

    echo ""

    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${CYAN}  📉 ERROR ANALYSIS${NC}"
    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

    echo -e "${YELLOW}⚠️  Custom(3007) - Market Volatility:${NC} $MARKET_ERRORS ${BLUE}(recent)${NC}"
    echo -e "${BLUE}    └─ This is NORMAL - pools changing state${NC}"

    echo ""

    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${BLUE}⌨️  Controls:${NC} Ctrl+C to exit | Refreshing every ${REFRESH_INTERVAL}s"
    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

    sleep "$REFRESH_INTERVAL"
done
