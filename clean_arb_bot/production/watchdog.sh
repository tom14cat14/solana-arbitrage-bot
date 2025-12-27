#!/bin/bash
# Arb Bot - Watchdog Script
# Auto-restarts bot on crashes with exponential backoff
# Implements killswitch for stuck loops and health monitoring

set -e

# Configuration
BOT_DIR="/home/tom14cat14/Arb_Bot/clean_arb_bot"
BINARY="$BOT_DIR/target/release/clean_arb_bot"
LOG_DIR="/tmp/arb_bot_logs"
HEALTH_CHECK_INTERVAL=30  # seconds
MAX_CONSECUTIVE_FAILURES=5
STUCK_THRESHOLD=300  # 5 minutes with no activity = stuck
KILLSWITCH_FILE="/tmp/arb_bot_killswitch"

# Create log directory
mkdir -p "$LOG_DIR"

# Initialize counters
FAILURE_COUNT=0
BACKOFF_SECONDS=1

# Health monitoring
LAST_ACTIVITY_TIME=$(date +%s)
LAST_SCAN_COUNT=0

echo "🐕 Watchdog started for Arb Bot"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Log directory: $LOG_DIR"
echo "Health check interval: ${HEALTH_CHECK_INTERVAL}s"
echo "Stuck threshold: ${STUCK_THRESHOLD}s"
echo "Killswitch file: $KILLSWITCH_FILE"
echo ""

# Remove old killswitch if exists
rm -f "$KILLSWITCH_FILE"

while true; do
    # Check killswitch
    if [ -f "$KILLSWITCH_FILE" ]; then
        echo "🛑 KILLSWITCH ACTIVATED - Stopping bot"
        echo "Reason: $(cat "$KILLSWITCH_FILE")"
        exit 0
    fi

    # Check if bot is already running (prevent duplicates)
    if pgrep -f "$BINARY" > /dev/null 2>&1; then
        # Bot is running, perform health check
        CURRENT_TIME=$(date +%s)
        TIME_SINCE_LAST_ACTIVITY=$((CURRENT_TIME - LAST_ACTIVITY_TIME))

        # Check for stuck loop (no activity for STUCK_THRESHOLD seconds)
        if [ "$TIME_SINCE_LAST_ACTIVITY" -gt "$STUCK_THRESHOLD" ]; then
            echo "⚠️  STUCK LOOP DETECTED - No activity for ${TIME_SINCE_LAST_ACTIVITY}s"
            echo "Killing stuck bot process..."
            pkill -9 -f "$BINARY" || true
            echo "Stuck loop detected - bot killed after ${TIME_SINCE_LAST_ACTIVITY}s inactivity" > "$KILLSWITCH_FILE"
            sleep 5

            # Reset killswitch and allow restart
            rm -f "$KILLSWITCH_FILE"
            FAILURE_COUNT=$((FAILURE_COUNT + 1))
            LAST_ACTIVITY_TIME=$(date +%s)
            continue
        fi

        # Check log for recent scans (activity indicator)
        if [ -f "$LOG_DIR/arb_bot.log" ]; then
            CURRENT_SCAN_COUNT=$(grep -c "Scanning.*tokens for triangle" "$LOG_DIR/arb_bot.log" 2>/dev/null || echo "0")

            if [ "$CURRENT_SCAN_COUNT" -gt "$LAST_SCAN_COUNT" ]; then
                # Activity detected
                LAST_ACTIVITY_TIME=$(date +%s)
                LAST_SCAN_COUNT=$CURRENT_SCAN_COUNT
                FAILURE_COUNT=0  # Reset failure counter on successful activity
            fi
        fi

        sleep "$HEALTH_CHECK_INTERVAL"
        continue
    fi

    # Bot is not running, check failure count
    if [ "$FAILURE_COUNT" -ge "$MAX_CONSECUTIVE_FAILURES" ]; then
        echo "🛑 MAX FAILURES REACHED ($MAX_CONSECUTIVE_FAILURES) - Activating killswitch"
        echo "Too many consecutive failures (${FAILURE_COUNT})" > "$KILLSWITCH_FILE"
        exit 1
    fi

    # Start bot
    TIMESTAMP=$(date '+%Y-%m-%d %H:%M:%S')
    echo "[$TIMESTAMP] 🚀 Starting Arb Bot (attempt $((FAILURE_COUNT + 1))/$MAX_CONSECUTIVE_FAILURES)"

    # Run bot and capture logs
    LOG_FILE="$LOG_DIR/arb_bot.log"

    env ENABLE_REAL_TRADING=true \
        PAPER_TRADING=false \
        SKIP_GHOST_POOL_CHECK=true \
        RUST_LOG=info \
        "$BINARY" >> "$LOG_FILE" 2>&1 &

    BOT_PID=$!
    echo "Bot started with PID: $BOT_PID"

    # Reset activity tracking
    LAST_ACTIVITY_TIME=$(date +%s)
    LAST_SCAN_COUNT=0

    # Wait for bot to initialize or crash
    sleep 5

    # Check if bot is still running
    if ! kill -0 "$BOT_PID" 2>/dev/null; then
        # Bot crashed immediately
        FAILURE_COUNT=$((FAILURE_COUNT + 1))
        echo "❌ Bot crashed immediately (failure $FAILURE_COUNT/$MAX_CONSECUTIVE_FAILURES)"

        # Exponential backoff
        echo "Waiting ${BACKOFF_SECONDS}s before retry..."
        sleep "$BACKOFF_SECONDS"
        BACKOFF_SECONDS=$((BACKOFF_SECONDS * 2))
        if [ "$BACKOFF_SECONDS" -gt 60 ]; then
            BACKOFF_SECONDS=60  # Cap at 1 minute
        fi
    else
        # Bot started successfully
        echo "✅ Bot started successfully"
        BACKOFF_SECONDS=1  # Reset backoff

        # Wait for bot process to exit
        wait "$BOT_PID"
        EXIT_CODE=$?

        TIMESTAMP=$(date '+%Y-%m-%d %H:%M:%S')
        echo "[$TIMESTAMP] ⚠️  Bot exited with code: $EXIT_CODE"

        FAILURE_COUNT=$((FAILURE_COUNT + 1))
        sleep 2  # Brief pause before restart
    fi
done
