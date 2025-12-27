#!/bin/bash
# Arb Bot - Production Stop Script
# Gracefully stops bot and kills tmux session

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'

SESSION_NAME="arb_bot"
BINARY="clean_arb_bot"
KILLSWITCH_FILE="/tmp/arb_bot_killswitch"

echo -e "${RED}🛑 Stopping Arb Bot${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# Activate killswitch
echo "Manual stop requested" > "$KILLSWITCH_FILE"
echo "✅ Killswitch activated"

# Kill bot process
if pgrep -f "$BINARY" > /dev/null 2>&1; then
    echo "Killing bot process..."
    pkill -15 -f "$BINARY" || true  # Try graceful shutdown first
    sleep 2

    # Force kill if still running
    if pgrep -f "$BINARY" > /dev/null 2>&1; then
        echo "Force killing bot process..."
        pkill -9 -f "$BINARY" || true
    fi
    echo "✅ Bot process killed"
else
    echo "Bot process not running"
fi

# Kill tmux session
if tmux has-session -t "$SESSION_NAME" 2>/dev/null; then
    tmux kill-session -t "$SESSION_NAME"
    echo "✅ Tmux session killed"
else
    echo "Tmux session not found"
fi

# Remove killswitch
rm -f "$KILLSWITCH_FILE"

echo ""
echo -e "${GREEN}✅ Arb Bot stopped successfully${NC}"
echo ""
echo "To restart: ./production/start_arb_bot.sh"
