#!/bin/bash
# Arb Bot - Status Check Script
# Shows current bot status, health, and recent activity

set -e

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

SESSION_NAME="arb_bot"
BINARY="clean_arb_bot"
LOG_FILE="/tmp/arb_bot_logs/arb_bot.log"
KILLSWITCH_FILE="/tmp/arb_bot_killswitch"

echo -e "${BLUE}📊 Arb Bot Status${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# Check killswitch
if [ -f "$KILLSWITCH_FILE" ]; then
    echo -e "${RED}🛑 KILLSWITCH ACTIVE${NC}"
    echo "Reason: $(cat $KILLSWITCH_FILE)"
    echo ""
fi

# Check tmux session
if tmux has-session -t "$SESSION_NAME" 2>/dev/null; then
    echo -e "${GREEN}✅ Tmux Session: RUNNING${NC}"

    # Get session info
    PANE_COUNT=$(tmux list-panes -t "$SESSION_NAME" 2>/dev/null | wc -l)
    echo "   Panes: $PANE_COUNT"
else
    echo -e "${RED}❌ Tmux Session: NOT RUNNING${NC}"
fi

echo ""

# Check bot process
if pgrep -f "$BINARY" > /dev/null 2>&1; then
    BOT_PID=$(pgrep -f "$BINARY")
    echo -e "${GREEN}✅ Bot Process: RUNNING${NC}"
    echo "   PID: $BOT_PID"

    # Get process info
    PS_INFO=$(ps -p $BOT_PID -o %cpu,%mem,etime,cmd --no-headers 2>/dev/null || echo "N/A")
    CPU=$(echo $PS_INFO | awk '{print $1}')
    MEM=$(echo $PS_INFO | awk '{print $2}')
    UPTIME=$(echo $PS_INFO | awk '{print $3}')

    echo "   CPU: ${CPU}%"
    echo "   Memory: ${MEM}%"
    echo "   Uptime: $UPTIME"
else
    echo -e "${RED}❌ Bot Process: NOT RUNNING${NC}"
fi

echo ""

# Check ShredStream service
if pgrep -f "shredstream_service" > /dev/null 2>&1; then
    echo -e "${GREEN}✅ ShredStream Service: RUNNING${NC}"
else
    echo -e "${RED}❌ ShredStream Service: NOT RUNNING${NC}"
fi

echo ""

# Recent activity
if [ -f "$LOG_FILE" ]; then
    echo -e "${BLUE}📈 Recent Activity (last 60 seconds):${NC}"

    # Count scans
    SCAN_COUNT=$(grep "Scanning.*tokens for triangle" "$LOG_FILE" 2>/dev/null | tail -40 | wc -l)
    echo "   Scans: $SCAN_COUNT"

    # Count opportunities detected
    OPP_COUNT=$(grep "Triangle opportunity:" "$LOG_FILE" 2>/dev/null | tail -100 | wc -l)
    echo "   Opportunities detected: $OPP_COUNT"

    # Count executions attempted
    EXEC_COUNT=$(grep "Executing triangle opportunity:" "$LOG_FILE" 2>/dev/null | tail -100 | wc -l)
    echo "   Execution attempts: $EXEC_COUNT"

    # Count successful executions
    SUCCESS_COUNT=$(grep "PROFITABLE.*Submitted bundle" "$LOG_FILE" 2>/dev/null | wc -l)
    echo "   Successful trades: $SUCCESS_COUNT"

    # Count rejections
    REJECT_COUNT=$(grep "no longer profitable after cost" "$LOG_FILE" 2>/dev/null | tail -100 | wc -l)
    echo "   Rejected (unprofitable): $REJECT_COUNT"

    echo ""

    # Last few log lines
    echo -e "${BLUE}📝 Last 5 Events:${NC}"
    grep -E "(opportunity|Executing|PROFITABLE|WARN|ERROR)" "$LOG_FILE" 2>/dev/null | tail -5 | sed 's/\x1b\[[0-9;]*m//g' | while read line; do
        echo "   $line"
    done
else
    echo -e "${YELLOW}⚠️  No log file found: $LOG_FILE${NC}"
fi

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Commands:"
echo "  📊 Attach to session: tmux attach -t $SESSION_NAME"
echo "  📄 View logs: tail -f $LOG_FILE"
echo "  🔴 Stop bot: ./production/stop_arb_bot.sh"
echo "  💊 Health check: ./production/health_check.sh"
