#!/bin/bash
# Arb Bot - Health Check Script v2
# Fixed RPC detection to avoid false positives

set -e

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

LOG_FILE="/tmp/arb_bot_logs/arb_bot.log"
KILLSWITCH_FILE="/tmp/arb_bot_killswitch"
HEALTH_SCORE=0
MAX_SCORE=10
ISSUES=()

echo -e "${BLUE}💊 Arb Bot Health Check v2${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# Check 1: Bot process running
if pgrep -f "clean_arb_bot" > /dev/null 2>&1; then
    echo -e "${GREEN}✅ Bot process is running${NC}"
    HEALTH_SCORE=$((HEALTH_SCORE + 2))
else
    echo -e "${RED}❌ Bot process is NOT running${NC}"
    ISSUES+=("Bot process down")
fi

# Check 2: ShredStream service running
if pgrep -f "shredstream_service" > /dev/null 2>&1; then
    echo -e "${GREEN}✅ ShredStream service is running${NC}"
    HEALTH_SCORE=$((HEALTH_SCORE + 2))
else
    echo -e "${RED}❌ ShredStream service is NOT running${NC}"
    ISSUES+=("ShredStream service down")
fi

# Check 3: No killswitch active
if [ ! -f "$KILLSWITCH_FILE" ]; then
    echo -e "${GREEN}✅ No killswitch active${NC}"
    HEALTH_SCORE=$((HEALTH_SCORE + 1))
else
    echo -e "${RED}❌ Killswitch is ACTIVE${NC}"
    echo "   Reason: $(cat $KILLSWITCH_FILE)"
    ISSUES+=("Killswitch active")
fi

# Check 4: Recent activity (last 60 seconds)
if [ -f "$LOG_FILE" ]; then
    RECENT_ACTIVITY=$(grep "Scanning.*tokens" "$LOG_FILE" 2>/dev/null | tail -40 | wc -l)

    if [ $RECENT_ACTIVITY -gt 0 ]; then
        echo -e "${GREEN}✅ Recent activity detected (${RECENT_ACTIVITY} scans)${NC}"
        HEALTH_SCORE=$((HEALTH_SCORE + 2))
    else
        echo -e "${YELLOW}⚠️  No recent activity in last 60s${NC}"
        ISSUES+=("No recent scans detected")
    fi
else
    echo -e "${RED}❌ Log file not found${NC}"
    ISSUES+=("Log file missing")
fi

# Check 5: No excessive errors (FIXED - only real errors)
if [ -f "$LOG_FILE" ]; then
    # Only count actual ERROR level logs, not warnings
    ERROR_COUNT=$(grep "ERROR" "$LOG_FILE" 2>/dev/null | grep -v "DEBUG\|INFO\|WARN" | tail -100 | wc -l)

    if [ $ERROR_COUNT -lt 5 ]; then
        echo -e "${GREEN}✅ Error rate normal (${ERROR_COUNT} errors in recent logs)${NC}"
        HEALTH_SCORE=$((HEALTH_SCORE + 1))
    elif [ $ERROR_COUNT -lt 20 ]; then
        echo -e "${YELLOW}⚠️  Moderate error rate (${ERROR_COUNT} errors)${NC}"
        ISSUES+=("Moderate error rate: $ERROR_COUNT")
    else
        echo -e "${RED}❌ High error rate (${ERROR_COUNT} errors)${NC}"
        ISSUES+=("High error rate: $ERROR_COUNT")
    fi
fi

# Check 6: RPC connectivity (FIXED - only real connection failures)
if [ -f "$LOG_FILE" ]; then
    # Only count actual RPC failures, not normal operation
    RPC_ERRORS=$(grep -E "RPC.*failed|connection refused|connection timeout|unable to connect" "$LOG_FILE" 2>/dev/null | tail -50 | wc -l)

    if [ $RPC_ERRORS -eq 0 ]; then
        echo -e "${GREEN}✅ RPC connectivity healthy (0 connection failures)${NC}"
        HEALTH_SCORE=$((HEALTH_SCORE + 1))
    elif [ $RPC_ERRORS -lt 5 ]; then
        echo -e "${GREEN}✅ RPC connectivity good (${RPC_ERRORS} minor issues)${NC}"
        HEALTH_SCORE=$((HEALTH_SCORE + 1))
    else
        echo -e "${YELLOW}⚠️  RPC connectivity issues (${RPC_ERRORS} connection failures)${NC}"
        ISSUES+=("RPC connectivity problems: $RPC_ERRORS")
    fi
fi

# Check 7: JITO bundle submission health
if [ -f "$LOG_FILE" ]; then
    BUNDLE_ERRORS=$(grep -i "JITO.*429\|bundle.*failed" "$LOG_FILE" 2>/dev/null | tail -50 | wc -l)

    if [ $BUNDLE_ERRORS -lt 5 ]; then
        echo -e "${GREEN}✅ JITO submission healthy${NC}"
        HEALTH_SCORE=$((HEALTH_SCORE + 1))
    else
        echo -e "${YELLOW}⚠️  JITO submission issues (${BUNDLE_ERRORS} recent errors)${NC}"
        ISSUES+=("JITO bundle problems: $BUNDLE_ERRORS")
    fi
fi

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# Overall health score
HEALTH_PERCENTAGE=$((HEALTH_SCORE * 100 / MAX_SCORE))

if [ $HEALTH_PERCENTAGE -ge 80 ]; then
    echo -e "${GREEN}🎉 Overall Health: EXCELLENT (${HEALTH_SCORE}/${MAX_SCORE} = ${HEALTH_PERCENTAGE}%)${NC}"
elif [ $HEALTH_PERCENTAGE -ge 60 ]; then
    echo -e "${YELLOW}⚠️  Overall Health: GOOD (${HEALTH_SCORE}/${MAX_SCORE} = ${HEALTH_PERCENTAGE}%)${NC}"
elif [ $HEALTH_PERCENTAGE -ge 40 ]; then
    echo -e "${YELLOW}⚠️  Overall Health: FAIR (${HEALTH_SCORE}/${MAX_SCORE} = ${HEALTH_PERCENTAGE}%)${NC}"
else
    echo -e "${RED}❌ Overall Health: POOR (${HEALTH_SCORE}/${MAX_SCORE} = ${HEALTH_PERCENTAGE}%)${NC}"
fi

# List issues
if [ ${#ISSUES[@]} -gt 0 ]; then
    echo ""
    echo -e "${RED}⚠️  Issues detected:${NC}"
    for issue in "${ISSUES[@]}"; do
        echo "   • $issue"
    done
    echo ""
    echo "Recommended actions:"
    if [[ " ${ISSUES[@]} " =~ "Bot process down" ]]; then
        echo "   → Restart bot: ./production/start_arb_bot.sh"
    fi
    if [[ " ${ISSUES[@]} " =~ "ShredStream service down" ]]; then
        echo "   → Restart ShredStream: cd /home/tom14cat14/Arb_Bot/shredstream_service && ~/.cargo/bin/cargo run --release"
    fi
    if [[ " ${ISSUES[@]} " =~ "Killswitch active" ]]; then
        echo "   → Clear killswitch: rm -f $KILLSWITCH_FILE"
        echo "   → Restart bot: ./production/start_arb_bot.sh"
    fi
fi

echo ""
exit $((MAX_SCORE - HEALTH_SCORE))  # Exit code = number of failed checks
