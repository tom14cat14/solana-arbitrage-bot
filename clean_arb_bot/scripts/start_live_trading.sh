#!/bin/bash
# Live Trading Starter with Logging
# REAL MONEY - USE WITH CAUTION

set -e

LOG_DIR="/home/tom14cat14/Arb_Bot/clean_arb_bot/logs"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
LOG_FILE="${LOG_DIR}/live_trading_${TIMESTAMP}.log"

# Create log directory
mkdir -p "$LOG_DIR"

echo "=========================================="
echo "🚨 STARTING LIVE TRADING - REAL MONEY 🚨"
echo "=========================================="
echo "Timestamp: $(date)"
echo "Log file: $LOG_FILE"
echo "Wallet: 9WrFdecsvMogYEtwjGrCBs4LrfnZhm9QKigD4CdcD3kA"
echo ""

# Check wallet balance
echo "Checking wallet balance..."
BALANCE=$(solana balance 9WrFdecsvMogYEtwjGrCBs4LrfnZhm9QKigD4CdcD3kA 2>/dev/null | awk '{print $1}')
echo "Current balance: $BALANCE SOL"
echo ""

# Verify ShredStream service is running
echo "Checking ShredStream service..."
if ! curl -s http://localhost:8080/health > /dev/null 2>&1; then
    echo "❌ ERROR: ShredStream service not running!"
    echo "Start it with: cd /home/tom14cat14/Arb_Bot/shredstream_service && cargo run --release"
    exit 1
fi
echo "✅ ShredStream service is running"
echo ""

# Show current configuration
echo "Current configuration:"
grep -E "(ENABLE_REAL_TRADING|PAPER_TRADING|CAPITAL_SOL|MAX_POSITION_SIZE_SOL|MIN_PROFIT_MARGIN_MULTIPLIER|MAX_DAILY_TRADES|DAILY_LOSS_LIMIT_SOL)" .env
echo ""

# Final confirmation
echo "=========================================="
echo "⚠️  FINAL WARNING ⚠️"
echo "=========================================="
echo "This will trade with REAL MONEY!"
echo "Press Ctrl+C within 10 seconds to abort..."
echo "=========================================="
sleep 10

echo ""
echo "Starting live trading bot..."
echo "Logging to: $LOG_FILE"
echo ""
echo "To stop: Press Ctrl+C or run: touch .killswitch"
echo ""

# Start the bot with logging
cd /home/tom14cat14/Arb_Bot/clean_arb_bot
env ENABLE_REAL_TRADING=true \
    PAPER_TRADING=false \
    RUST_LOG=info \
    ~/.cargo/bin/cargo run --release 2>&1 | tee "$LOG_FILE"
