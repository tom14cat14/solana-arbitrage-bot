#!/bin/bash
# Enhanced Live Monitoring Dashboard for Arbitrage Bot
# Shows detailed opportunity calculations and JITO submission tracking
# Usage: ./monitor_dashboard_detailed.sh

LOG_FILE="../logs/arb_bot.log"
REFRESH_INTERVAL=3  # seconds

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
MAGENTA='\033[0;35m'
NC='\033[0m' # No Color

# Function to update screen - no clearing, just repositions cursor
clear_screen() {
    # Just move cursor to top-left without clearing
    # Content will be overwritten in place
    printf '\033[H'
}

# Function to get recent stats
get_stats() {
    # Time window: last 500 lines for recent activity
    RECENT_LOGS=$(tail -500 "$LOG_FILE")

    # Count opportunities (sanitize to ensure single integer)
    OPPS_DETECTED=$(echo "$RECENT_LOGS" | grep -c "Triangle opportunity:" 2>/dev/null | head -1 | tr -d '\n' || echo "0")
    OPPS_DETECTED=${OPPS_DETECTED:-0}

    # Count bundle activity (sanitize to ensure single integer)
    BUNDLES_QUEUED=$(echo "$RECENT_LOGS" | grep -c "Bundle queued:" 2>/dev/null | head -1 | tr -d '\n' || echo "0")
    BUNDLES_QUEUED=${BUNDLES_QUEUED:-0}
    JITO_SUBMISSIONS=$(echo "$RECENT_LOGS" | grep -c "Submitting SECURE Jito bundle" 2>/dev/null | head -1 | tr -d '\n' || echo "0")
    JITO_SUBMISSIONS=${JITO_SUBMISSIONS:-0}
    JITO_SUCCESS=$(grep -c "JITO bundle submitted successfully" "$LOG_FILE" 2>/dev/null | head -1 | tr -d '\n' || echo "0")
    JITO_SUCCESS=${JITO_SUCCESS:-0}

    # Count rejections (sanitize to ensure single integer)
    COST_REJECTED=$(echo "$RECENT_LOGS" | grep -c "no longer profitable after cost" 2>/dev/null | head -1 | tr -d '\n' || echo "0")
    COST_REJECTED=${COST_REJECTED:-0}
    RATE_LIMITED=$(echo "$RECENT_LOGS" | grep -c "429 Rate Limit" 2>/dev/null | head -1 | tr -d '\n' || echo "0")
    RATE_LIMITED=${RATE_LIMITED:-0}

    # Count errors (sanitize to ensure single integer)
    CRITICAL_ERRORS=$(grep -c "Custom(101)" "$LOG_FILE" 2>/dev/null | head -1 | tr -d '\n' || echo "0")
    CRITICAL_ERRORS=${CRITICAL_ERRORS:-0}
    MARKET_ERRORS=$(echo "$RECENT_LOGS" | grep -c "Custom(3007)" 2>/dev/null | head -1 | tr -d '\n' || echo "0")
    MARKET_ERRORS=${MARKET_ERRORS:-0}

    # Get last 3 profitable opportunities with full details
    LAST_PROFITS=$(echo "$RECENT_LOGS" | grep -B2 "Net profit:" | tail -12)

    # Get recent JITO activity
    JITO_ACTIVITY=$(echo "$RECENT_LOGS" | grep -E "(Bundle queued|429 Rate Limit|Submitting SECURE|bundle submitted)" | tail -8)

    # Get recent rejections
    RECENT_REJECTIONS=$(echo "$RECENT_LOGS" | grep -A1 "no longer profitable" | tail -6)
}

# Main display loop
while true; do
    clear_screen
    get_stats

    echo -e "${CYAN}═══════════════════════════════════════════════════════════════════════════${NC}"
    echo -e "${CYAN}              ARBITRAGE BOT - DETAILED MONITORING DASHBOARD${NC}"
    echo -e "${CYAN}═══════════════════════════════════════════════════════════════════════════${NC}"
    echo ""
    echo -e "${BLUE}📊 Last Updated:${NC} $(date '+%Y-%m-%d %H:%M:%S')"
    echo -e "${BLUE}📂 Log File:${NC} $LOG_FILE"
    echo ""

    # ═══════════════════════════════════════════════════════════════════════
    # CRITICAL HEALTH CHECK
    # ═══════════════════════════════════════════════════════════════════════
    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${CYAN}  🏥 SYSTEM HEALTH${NC}"
    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

    if [ "$CRITICAL_ERRORS" -eq 0 ]; then
        echo -e "${GREEN}✅ Code Bugs (Custom 101):${NC} $CRITICAL_ERRORS ${GREEN}PERFECT${NC}"
    else
        echo -e "${RED}❌ Code Bugs (Custom 101):${NC} $CRITICAL_ERRORS ${RED}⚠️ STOP BOT!${NC}"
    fi

    echo -e "${BLUE}📊 Market Volatility (Custom 3007):${NC} $MARKET_ERRORS ${BLUE}(normal)${NC}"
    echo ""

    # ═══════════════════════════════════════════════════════════════════════
    # OPPORTUNITY PIPELINE
    # ═══════════════════════════════════════════════════════════════════════
    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${CYAN}  🔄 OPPORTUNITY PIPELINE (Last 500 lines)${NC}"
    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

    echo -e "${YELLOW}🔍 Opportunities Detected:${NC} $OPPS_DETECTED"
    echo -e "${MAGENTA}📦 Bundles Queued:${NC} $BUNDLES_QUEUED"
    echo -e "${BLUE}🚀 JITO Attempts:${NC} $JITO_SUBMISSIONS"
    echo -e "${GREEN}✅ JITO Success (All Time):${NC} $JITO_SUCCESS"
    echo ""
    echo -e "${RED}❌ Cost Rejected:${NC} $COST_REJECTED ${BLUE}(unprofitable after fees)${NC}"
    echo -e "${YELLOW}⏸️  Rate Limited:${NC} $RATE_LIMITED ${BLUE}(JITO 429 - network busy)${NC}"
    echo ""

    # ═══════════════════════════════════════════════════════════════════════
    # PROFITABLE OPPORTUNITIES WITH CALCULATIONS
    # ═══════════════════════════════════════════════════════════════════════
    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${CYAN}  💰 RECENT PROFITABLE OPPORTUNITIES (Last 3)${NC}"
    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

    if [ -n "$LAST_PROFITS" ]; then
        echo "$LAST_PROFITS" | while read -r line; do
            # Check for opportunity path
            if echo "$line" | grep -q "Triangle opportunity:"; then
                path=$(echo "$line" | grep -oP '\[.*?\]' | head -1)
                gross=$(echo "$line" | grep -oP '→ \K[0-9.]+')
                echo -e "\n${YELLOW}🔺 Path:${NC} $path → ${YELLOW}Gross: ${gross} SOL${NC}"
            fi

            # Check for cost breakdown
            if echo "$line" | grep -q "DEX fees:"; then
                dex=$(echo "$line" | grep -oP 'DEX fees: \K[0-9.]+')
                jito=$(echo "$line" | grep -oP 'JITO tip: \K[0-9.]+')
                gas=$(echo "$line" | grep -oP 'Gas: \K[0-9.]+')
                echo -e "   ${BLUE}└─ Costs:${NC} DEX ${dex} + JITO ${jito} + Gas ${gas}"
            fi

            # Check for net profit
            if echo "$line" | grep -q "Net profit:"; then
                net=$(echo "$line" | grep -oP 'Net profit: \K[0-9.]+')
                retention=$(echo "$line" | grep -oP '\(\K[0-9.]+')
                echo -e "   ${GREEN}└─ Net Profit: ${net} SOL${NC} (${retention}% retention)"
            fi
        done
    else
        echo -e "${YELLOW}⏳ Waiting for profitable opportunities...${NC}"
    fi
    echo ""

    # ═══════════════════════════════════════════════════════════════════════
    # RECENT REJECTIONS
    # ═══════════════════════════════════════════════════════════════════════
    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${CYAN}  🚫 RECENT REJECTIONS (Why Not Executed)${NC}"
    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

    if [ -n "$RECENT_REJECTIONS" ]; then
        echo "$RECENT_REJECTIONS" | grep -E "(no longer profitable|Gross profit:|Total costs:)" | tail -6 | while read -r line; do
            if echo "$line" | grep -q "Gross profit:"; then
                gross=$(echo "$line" | grep -oP 'Gross profit: \K[0-9.]+')
                echo -e "${YELLOW}   Gross: ${gross} SOL${NC}"
            elif echo "$line" | grep -q "Total costs:"; then
                costs=$(echo "$line" | grep -oP 'Total costs: \K[0-9.]+')
                echo -e "${RED}   Costs: ${costs} SOL${NC} ${BLUE}(costs > profit)${NC}"
            fi
        done
    else
        echo -e "${GREEN}✅ No recent rejections${NC}"
    fi
    echo ""

    # ═══════════════════════════════════════════════════════════════════════
    # JITO SUBMISSION ACTIVITY
    # ═══════════════════════════════════════════════════════════════════════
    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${CYAN}  🎯 JITO SUBMISSION ACTIVITY (Live Feed)${NC}"
    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

    if [ -n "$JITO_ACTIVITY" ]; then
        echo "$JITO_ACTIVITY" | while read -r line; do
            # Strip ANSI codes
            clean_line=$(echo "$line" | sed 's/\x1b\[[0-9;]*m//g')

            if echo "$clean_line" | grep -q "Bundle queued"; then
                pair=$(echo "$clean_line" | grep -oP '2-leg: \K.*')
                echo -e "${MAGENTA}📦 Queued:${NC} $pair"
            elif echo "$clean_line" | grep -q "Submitting SECURE"; then
                echo -e "${BLUE}🚀 Submitting to JITO...${NC}"
            elif echo "$clean_line" | grep -q "429 Rate Limit"; then
                echo -e "${YELLOW}⏸️  JITO Rate Limited${NC} ${BLUE}(dropping stale)${NC}"
            elif echo "$clean_line" | grep -q "bundle submitted"; then
                echo -e "${GREEN}✅ JITO Success!${NC}"
            fi
        done
    else
        echo -e "${YELLOW}⏳ Waiting for JITO activity...${NC}"
    fi
    echo ""

    # ═══════════════════════════════════════════════════════════════════════
    # STATUS SUMMARY
    # ═══════════════════════════════════════════════════════════════════════
    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${CYAN}  📊 STATUS SUMMARY${NC}"
    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

    if [ "$JITO_SUCCESS" -gt 0 ]; then
        echo -e "${GREEN}🎉 Bot has successfully executed $JITO_SUCCESS trades on-chain!${NC}"
    elif [ "$RATE_LIMITED" -gt "$COST_REJECTED" ]; then
        echo -e "${YELLOW}⏸️  Primary bottleneck: JITO rate limiting (network congestion)${NC}"
        echo -e "${BLUE}   └─ Bot is healthy, waiting for JITO to accept connections${NC}"
    elif [ "$COST_REJECTED" -gt 0 ]; then
        echo -e "${YELLOW}💰 Primary bottleneck: Opportunities too small after costs${NC}"
        echo -e "${BLUE}   └─ Need gross profit > 0.02 SOL to be profitable${NC}"
    elif [ "$OPPS_DETECTED" -gt 0 ]; then
        echo -e "${GREEN}🔍 Bot is actively scanning, finding opportunities${NC}"
        echo -e "${BLUE}   └─ Waiting for profitable arbitrage windows${NC}"
    else
        echo -e "${BLUE}⏳ Bot is running, monitoring markets...${NC}"
    fi

    echo ""
    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${BLUE}⌨️  Controls:${NC} Ctrl+C to exit | Auto-refresh every ${REFRESH_INTERVAL}s"
    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

    sleep "$REFRESH_INTERVAL"
done
