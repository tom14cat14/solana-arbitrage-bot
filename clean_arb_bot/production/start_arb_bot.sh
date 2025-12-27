#!/bin/bash
# Arb Bot - Production Start Script with Tmux
# Starts bot in tmux session with auto-restart and safety features

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

BOT_DIR="/home/tom14cat14/Arb_Bot/clean_arb_bot"
SESSION_NAME="arb_bot"
LOG_DIR="/tmp/arb_bot_logs"
BINARY="$BOT_DIR/target/release/clean_arb_bot"

# Create log directory
mkdir -p "$LOG_DIR"

echo -e "${GREEN}🚀 Starting Arb Bot in Production Mode${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# Check if binary exists
if [ ! -f "$BINARY" ]; then
    echo -e "${RED}❌ Binary not found: $BINARY${NC}"
    echo "Building bot..."
    cd "$BOT_DIR"
    ~/.cargo/bin/cargo build --release --quiet
fi

# Check if tmux session already exists
if tmux has-session -t "$SESSION_NAME" 2>/dev/null; then
    echo -e "${YELLOW}⚠️  Tmux session '$SESSION_NAME' already exists${NC}"
    echo "Options:"
    echo "  1. Attach to existing session: tmux attach -t $SESSION_NAME"
    echo "  2. Kill and restart: ./production/stop_arb_bot.sh && ./production/start_arb_bot.sh"
    exit 1
fi

# Create new tmux session (detached)
tmux new-session -d -s "$SESSION_NAME" -c "$BOT_DIR"

# Set up the environment and start the bot with watchdog
tmux send-keys -t "$SESSION_NAME" "cd \"$BOT_DIR\"" C-m
tmux send-keys -t "$SESSION_NAME" "source .env 2>/dev/null || true" C-m
tmux send-keys -t "$SESSION_NAME" "./production/watchdog.sh" C-m

echo -e "${GREEN}✅ Arb Bot started in tmux session: $SESSION_NAME${NC}"
echo ""
echo "Management Commands:"
echo "  📊 View logs:    tmux attach -t $SESSION_NAME"
echo "  📈 Status:       ./production/status_arb_bot.sh"
echo "  🔴 Stop:         ./production/stop_arb_bot.sh"
echo "  💊 Health:       ./production/health_check.sh"
echo ""
echo "Logs are saved to: $LOG_DIR/"
echo ""
echo -e "${YELLOW}⚠️  Bot is running with REAL MONEY in LIVE mode${NC}"
echo "Monitor frequently: tail -f $LOG_DIR/arb_bot.log"
