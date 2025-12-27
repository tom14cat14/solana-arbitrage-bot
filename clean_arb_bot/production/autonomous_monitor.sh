#!/bin/bash

# Autonomous Arb Bot Monitor with Auto-Restart, Killswitch, and Full Monitoring
# Runs indefinitely with health checks, balance tracking, and error recovery

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
ARB_BOT_DIR="/home/tom14cat14/Arb_Bot/clean_arb_bot"
SHREDSTREAM_DIR="/home/tom14cat14/Arb_Bot/shredstream_service"
LOG_DIR="/home/tom14cat14/Arb_Bot/logs"
KILLSWITCH_FILE="/home/tom14cat14/Arb_Bot/KILLSWITCH"
HEALTH_CHECK_INTERVAL=30  # Check health every 30 seconds
RESTART_DELAY=5           # Wait 5 seconds before restart

# Wallet address for balance monitoring
WALLET="9WrFdecsvMogYEtwjGrCBs4LrfnZhm9QKigD4CdcD3kA"

# Create log directory
mkdir -p "$LOG_DIR"

# Log files
MONITOR_LOG="$LOG_DIR/autonomous_monitor.log"
ARB_BOT_LOG="$LOG_DIR/arb_bot.log"
SHREDSTREAM_LOG="$LOG_DIR/shredstream_service.log"
HEALTH_LOG="$LOG_DIR/health_checks.log"
BALANCE_LOG="$LOG_DIR/balance_tracking.log"

# PIDs
ARB_BOT_PID=""
SHREDSTREAM_PID=""

# Initialize logs
echo "========================================" | tee -a "$MONITOR_LOG"
echo "$(date '+%Y-%m-%d %H:%M:%S') - Autonomous Monitor Started" | tee -a "$MONITOR_LOG"
echo "========================================" | tee -a "$MONITOR_LOG"

# Log function
log() {
    echo -e "${GREEN}[$(date '+%Y-%m-%d %H:%M:%S')]${NC} $1" | tee -a "$MONITOR_LOG"
}

log_error() {
    echo -e "${RED}[$(date '+%Y-%m-%d %H:%M:%S')] ERROR:${NC} $1" | tee -a "$MONITOR_LOG"
}

log_warn() {
    echo -e "${YELLOW}[$(date '+%Y-%m-%d %H:%M:%S')] WARNING:${NC} $1" | tee -a "$MONITOR_LOG"
}

log_info() {
    echo -e "${BLUE}[$(date '+%Y-%m-%d %H:%M:%S')] INFO:${NC} $1" | tee -a "$MONITOR_LOG"
}

# Check killswitch
check_killswitch() {
    if [ -f "$KILLSWITCH_FILE" ]; then
        log_error "KILLSWITCH ACTIVATED! Stopping all bots..."
        stop_all_bots
        log_error "All bots stopped. Remove $KILLSWITCH_FILE to resume."
        exit 0
    fi
}

# Stop all bots gracefully
stop_all_bots() {
    log "Stopping all bots..."

    if [ -n "$ARB_BOT_PID" ] && kill -0 "$ARB_BOT_PID" 2>/dev/null; then
        log "Stopping arb bot (PID: $ARB_BOT_PID)"
        kill "$ARB_BOT_PID" 2>/dev/null || true
        wait "$ARB_BOT_PID" 2>/dev/null || true
    fi

    if [ -n "$SHREDSTREAM_PID" ] && kill -0 "$SHREDSTREAM_PID" 2>/dev/null; then
        log "Stopping ShredStream service (PID: $SHREDSTREAM_PID)"
        kill "$SHREDSTREAM_PID" 2>/dev/null || true
        wait "$SHREDSTREAM_PID" 2>/dev/null || true
    fi

    # Cleanup any orphaned processes
    pkill -f "shredstream_service" 2>/dev/null || true
    pkill -f "clean_arb_bot" 2>/dev/null || true

    log "All bots stopped"
}

# Start ShredStream service
start_shredstream() {
    log "Starting ShredStream service..."

    # Run from shredstream_service directory so relative paths work (whitelisted_pools.json)
    cd "$SHREDSTREAM_DIR"
    env SHREDS_ENDPOINT=https://shreds-ny6-1.erpc.global \
        RUST_LOG=info \
        ./target/release/shredstream_service \
        >> "$SHREDSTREAM_LOG" 2>&1 &

    SHREDSTREAM_PID=$!
    log "✅ ShredStream service started (PID: $SHREDSTREAM_PID)"

    # Wait for service to initialize
    sleep 5

    # Verify it's running
    if ! kill -0 "$SHREDSTREAM_PID" 2>/dev/null; then
        log_error "ShredStream service failed to start"
        tail -20 "$SHREDSTREAM_LOG"
        return 1
    fi

    log_info "ShredStream service healthy"
    return 0
}

# Start Arb Bot
start_arb_bot() {
    log "Starting Arb Bot with REAL TRADING ENABLED..."

    cd "$ARB_BOT_DIR"

    # Load .env file for wallet configuration
    if [ -f "$ARB_BOT_DIR/.env" ]; then
        set -a  # Automatically export all variables
        source "$ARB_BOT_DIR/.env"
        set +a
        log "✅ Loaded configuration from .env"
    else
        log_error ".env file not found - JITO bundles will be disabled!"
    fi

    env ENABLE_REAL_TRADING=true \
        PAPER_TRADING=false \
        RUST_LOG=info \
        ./target/release/clean_arb_bot \
        >> "$ARB_BOT_LOG" 2>&1 &

    ARB_BOT_PID=$!
    log "✅ Arb Bot started (PID: $ARB_BOT_PID)"
    log_warn "🔥 REAL MONEY TRADING ACTIVE 🔥"

    # Wait for bot to initialize
    sleep 3

    # Verify it's running
    if ! kill -0 "$ARB_BOT_PID" 2>/dev/null; then
        log_error "Arb Bot failed to start"
        tail -20 "$ARB_BOT_LOG"
        return 1
    fi

    log_info "Arb Bot healthy"
    return 0
}

# Health check for ShredStream
check_shredstream_health() {
    if [ -z "$SHREDSTREAM_PID" ] || ! kill -0 "$SHREDSTREAM_PID" 2>/dev/null; then
        log_error "ShredStream service crashed!"
        return 1
    fi

    # Check if service is responding (HTTP health check)
    if ! curl -s http://localhost:8080/api/health > /dev/null 2>&1; then
        log_warn "ShredStream service not responding"
        return 1
    fi

    return 0
}

# Health check for Arb Bot
check_arb_bot_health() {
    if [ -z "$ARB_BOT_PID" ] || ! kill -0 "$ARB_BOT_PID" 2>/dev/null; then
        log_error "Arb Bot crashed!"
        return 1
    fi

    # Check for recent activity in logs (last 60 seconds)
    local recent_activity
    recent_activity=$(tail -100 "$ARB_BOT_LOG" | grep -c "$(date +%Y-%m-%d)" || echo "0")
    if [ "$recent_activity" -eq 0 ]; then
        log_warn "Arb Bot appears frozen (no recent log activity)"
        return 1
    fi

    return 0
}

# Check wallet balance
check_balance() {
    local balance
    balance=$(solana balance "$WALLET" 2>/dev/null | awk '{print $1}' || echo "ERROR")

    if [ "$balance" = "ERROR" ]; then
        log_warn "Failed to check wallet balance"
        return
    fi

    echo "$(date '+%Y-%m-%d %H:%M:%S'),$balance" >> "$BALANCE_LOG"
    log_info "Wallet balance: $balance SOL"

    # Alert if balance is low
    if (( $(echo "$balance < 0.1" | bc -l) )); then
        log_error "⚠️ LOW BALANCE ALERT: $balance SOL"
    fi
}

# Monitor log errors
check_log_errors() {
    local recent_errors
    recent_errors=$(tail -50 "$ARB_BOT_LOG" | grep -i "error\|panic\|fatal" | tail -5)

    if [ -n "$recent_errors" ]; then
        log_warn "Recent errors detected:"
        echo "$recent_errors" | while read -r line; do
            log_warn "  $line"
        done
    fi
}

# Main monitoring loop
monitor_loop() {
    local loop_count=0
    local last_balance_check=0
    local balance_check_interval=300  # Check balance every 5 minutes

    while true; do
        # Check killswitch first
        check_killswitch

        # Health checks
        local shredstream_healthy=true
        local arb_bot_healthy=true

        if ! check_shredstream_health; then
            shredstream_healthy=false
            log_error "ShredStream health check failed"
        fi

        if ! check_arb_bot_health; then
            arb_bot_healthy=false
            log_error "Arb Bot health check failed"
        fi

        # Log health status
        if $shredstream_healthy && $arb_bot_healthy; then
            echo "$(date '+%Y-%m-%d %H:%M:%S'),OK,OK" >> "$HEALTH_LOG"
        else
            echo "$(date '+%Y-%m-%d %H:%M:%S'),$shredstream_healthy,$arb_bot_healthy" >> "$HEALTH_LOG"
        fi

        # Restart if needed
        if ! $shredstream_healthy; then
            log_error "Restarting ShredStream service..."
            if [ -n "$SHREDSTREAM_PID" ]; then
                kill "$SHREDSTREAM_PID" 2>/dev/null || true
                wait "$SHREDSTREAM_PID" 2>/dev/null || true
            fi
            sleep "$RESTART_DELAY"
            start_shredstream || log_error "Failed to restart ShredStream"
        fi

        if ! $arb_bot_healthy; then
            log_error "Restarting Arb Bot..."
            if [ -n "$ARB_BOT_PID" ]; then
                kill "$ARB_BOT_PID" 2>/dev/null || true
                wait "$ARB_BOT_PID" 2>/dev/null || true
            fi
            sleep "$RESTART_DELAY"
            start_arb_bot || log_error "Failed to restart Arb Bot"
        fi

        # Periodic balance check
        local now
        now=$(date +%s)
        if [ $((now - last_balance_check)) -ge "$balance_check_interval" ]; then
            check_balance
            last_balance_check=$now
        fi

        # Check for errors every 10 loops
        if [ $((loop_count % 10)) -eq 0 ]; then
            check_log_errors
        fi

        # Increment loop counter
        loop_count=$((loop_count + 1))

        # Sleep before next check
        sleep "$HEALTH_CHECK_INTERVAL"
    done
}

# Trap signals for graceful shutdown
trap 'log "Received shutdown signal"; stop_all_bots; exit 0' SIGINT SIGTERM

# Start services
log "========================================"
log "Starting services..."
log "========================================"

start_shredstream || { log_error "Failed to start ShredStream"; exit 1; }
sleep 2
start_arb_bot || { log_error "Failed to start Arb Bot"; exit 1; }

log "========================================"
log "All services started successfully"
log "Entering monitoring loop..."
log "========================================"
log_info "Health checks every $HEALTH_CHECK_INTERVAL seconds"
log_info "Balance checks every 5 minutes"
log_info "Auto-restart enabled"
log_info "Killswitch: $KILLSWITCH_FILE"
log "========================================"

# Initial balance check
check_balance

# Enter monitoring loop
monitor_loop
