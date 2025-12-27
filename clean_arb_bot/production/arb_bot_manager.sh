#!/bin/bash

# Clean Arb Bot Production Manager
# Handles auto-restart, crash detection, and killswitch

set -euo pipefail

# Configuration
BOT_NAME="clean_arb_bot"
BOT_DIR="/home/tom14cat14/Arb_Bot/clean_arb_bot"
LOG_DIR="$BOT_DIR/logs"
PIDFILE="$BOT_DIR/.arb_bot.pid"
KILLSWITCH_FILE="$BOT_DIR/.killswitch"
RESTART_COUNT_FILE="$BOT_DIR/.restart_count"
LAST_RESTART_FILE="$BOT_DIR/.last_restart"
MAX_RESTARTS_PER_HOUR=10
BACKOFF_BASE=5  # Base backoff in seconds
BACKOFF_MAX=300 # Max backoff in seconds (5 minutes)

# Create log directory if it doesn't exist
mkdir -p "$LOG_DIR"

# Color codes removed - not used in this script (SC2034)

# Logging function
log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $1" | tee -a "$LOG_DIR/manager.log"
}

# Check if killswitch is engaged
check_killswitch() {
    if [ -f "$KILLSWITCH_FILE" ]; then
        log "🚨 KILLSWITCH ENGAGED - Bot will not start"
        log "   Remove $KILLSWITCH_FILE to allow starting"
        exit 1
    fi
}

# Reset restart counter if enough time has passed
reset_restart_counter_if_needed() {
    if [ -f "$LAST_RESTART_FILE" ]; then
        last_restart=$(cat "$LAST_RESTART_FILE")
        current_time=$(date +%s)
        time_diff=$((current_time - last_restart))

        # Reset counter if more than 1 hour has passed
        if [ $time_diff -gt 3600 ]; then
            echo "0" > "$RESTART_COUNT_FILE"
            log "✅ Restart counter reset (>1 hour since last restart)"
        fi
    fi
}

# Check restart limits
check_restart_limits() {
    local restart_count=0

    if [ -f "$RESTART_COUNT_FILE" ]; then
        restart_count=$(cat "$RESTART_COUNT_FILE")
    fi

    if [ "$restart_count" -ge "$MAX_RESTARTS_PER_HOUR" ]; then
        log "🚨 CRASH LOOP DETECTED - Engaging killswitch"
        log "   $restart_count restarts in the last hour (max: $MAX_RESTARTS_PER_HOUR)"
        touch "$KILLSWITCH_FILE"

        # Send alert (you can add Discord/Telegram webhook here)
        echo "ALERT: Arb Bot crash loop detected. Killswitch engaged." | \
            tee -a "$LOG_DIR/alerts.log"

        exit 1
    fi
}

# Calculate backoff time
calculate_backoff() {
    local restart_count=0

    if [ -f "$RESTART_COUNT_FILE" ]; then
        restart_count=$(cat "$RESTART_COUNT_FILE")
    fi

    # Exponential backoff: 5s, 10s, 20s, 40s, 80s, 160s, max 300s
    local backoff=$((BACKOFF_BASE * (2 ** restart_count)))

    if [ "$backoff" -gt "$BACKOFF_MAX" ]; then
        backoff=$BACKOFF_MAX
    fi

    echo $backoff
}

# Increment restart counter
increment_restart_counter() {
    local count=0

    if [ -f "$RESTART_COUNT_FILE" ]; then
        count=$(cat "$RESTART_COUNT_FILE")
    fi

    count=$((count + 1))
    echo $count > "$RESTART_COUNT_FILE"
    date +%s > "$LAST_RESTART_FILE"

    log "📊 Restart #$count in current period"
}

# Start the bot
start_bot() {
    log "🚀 Starting $BOT_NAME..."

    # Check preconditions
    check_killswitch
    reset_restart_counter_if_needed
    check_restart_limits

    # Rotate logs if they're too large (>100MB)
    if [ -f "$LOG_DIR/arb_bot.log" ]; then
        log_size=$(stat -c%s "$LOG_DIR/arb_bot.log" 2>/dev/null || echo 0)
        if [ $log_size -gt 104857600 ]; then
            mv "$LOG_DIR/arb_bot.log" "$LOG_DIR/arb_bot.$(date +%Y%m%d_%H%M%S).log"
            log "📋 Rotated large log file"
        fi
    fi

    # Set environment variables
    export ENABLE_REAL_TRADING=true
    export PAPER_TRADING=false
    export RUST_LOG=info
    export RUST_BACKTRACE=1

    # Start the bot and capture PID
    cd "$BOT_DIR"
    nohup ./target/release/clean_arb_bot \
        >> "$LOG_DIR/arb_bot.log" 2>&1 &

    local pid=$!
    echo $pid > "$PIDFILE"

    log "✅ Bot started with PID: $pid"

    # Monitor the bot
    monitor_bot $pid
}

# Monitor bot and restart if needed
monitor_bot() {
    local pid=$1
    local consecutive_success=0
    local last_health_check
    last_health_check=$(date +%s)

    log "👁️ Monitoring bot PID: $pid"

    while true; do
        # Check if process is still running
        if ! kill -0 "$pid" 2>/dev/null; then
            log "⚠️ Bot process died (PID: $pid)"

            # Check exit code if available
            wait "$pid"
            exit_code=$?
            log "   Exit code: $exit_code"

            # Clean restart if exit was clean (0) or signal (130 = Ctrl+C)
            if [ "$exit_code" -eq 0 ] || [ "$exit_code" -eq 130 ]; then
                log "✅ Clean exit detected"
                echo "0" > "$RESTART_COUNT_FILE"
                break
            fi

            # Crashed - implement backoff
            increment_restart_counter
            backoff=$(calculate_backoff)

            log "⏱️ Waiting ${backoff}s before restart (exponential backoff)..."
            sleep "$backoff"

            # Restart
            start_bot
            break
        fi

        # Health check every 30 seconds
        current_time=$(date +%s)
        if [ $((current_time - last_health_check)) -ge 30 ]; then
            # Check if bot is actually processing (not hung)
            if tail -n 100 "$LOG_DIR/arb_bot.log" | grep -q "Scan complete"; then
                consecutive_success=$((consecutive_success + 1))

                # Reset restart counter after 10 consecutive successful health checks (5 minutes)
                if [ $consecutive_success -ge 10 ]; then
                    echo "0" > "$RESTART_COUNT_FILE"
                    consecutive_success=0
                    log "✅ Bot healthy for 5+ minutes, restart counter reset"
                fi
            else
                consecutive_success=0
            fi

            last_health_check=$current_time
        fi

        # Check for killswitch every 5 seconds
        if [ -f "$KILLSWITCH_FILE" ]; then
            log "🛑 Killswitch detected - stopping bot"
            kill "$pid" 2>/dev/null || true
            break
        fi

        # Check for emergency stop file
        if [ -f "$BOT_DIR/.emergency_stop" ]; then
            log "🚨 Emergency stop file detected - stopping bot"
            kill "$pid" 2>/dev/null || true
            rm -f "$BOT_DIR/.emergency_stop"
            break
        fi

        sleep 5
    done

    rm -f "$PIDFILE"
    log "👋 Manager exiting"
}

# Signal handlers
trap 'log "Received SIGTERM/SIGINT, shutting down..."; exit 0' SIGTERM SIGINT

# Main
main() {
    log "═══════════════════════════════════════"
    log "Clean Arb Bot Production Manager Started"
    log "═══════════════════════════════════════"

    # Start the bot
    start_bot
}

# Run main function
main "$@"