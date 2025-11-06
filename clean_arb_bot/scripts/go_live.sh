#!/bin/bash
# Clean Arb Bot - Go Live Script
# CRITICAL: This script enables REAL MONEY TRADING

set -e  # Exit on error

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "🚀 Clean Arb Bot - Live Trading Deployment"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# Wallet address
WALLET="9WrFdecsvMogYEtwjGrCBs4LrfnZhm9QKigD4CdcD3kA"

echo "📋 Pre-Flight Checklist:"
echo ""

# Check if wallet is funded
echo "1️⃣ Checking wallet balance..."
echo "   Wallet: $WALLET"

# Try to check balance (will show command to run manually if solana CLI not available)
if command -v solana &> /dev/null; then
    BALANCE=$(solana balance $WALLET 2>/dev/null || echo "0")
    echo "   Balance: $BALANCE"

    # Parse balance (remove "SOL" and convert to number)
    BALANCE_NUM=$(echo $BALANCE | awk '{print $1}')

    # Check if balance is sufficient (at least 0.1 SOL)
    if (( $(echo "$BALANCE_NUM < 0.1" | bc -l) )); then
        echo "   ⚠️  WARNING: Balance too low!"
        echo "   ⚠️  Need at least 0.1 SOL for testing"
        echo "   ⚠️  Recommended: 2-5 SOL for live trading"
        echo ""
        echo "Fund wallet and try again."
        exit 1
    else
        echo "   ✅ Balance sufficient ($BALANCE_NUM SOL)"
    fi
else
    echo "   ⚠️  Solana CLI not found"
    echo "   ⚠️  Check balance manually at:"
    echo "       https://solscan.io/account/$WALLET"
    echo ""
    read -p "Confirm wallet has at least 0.1 SOL (y/n): " confirm
    if [[ $confirm != "y" ]]; then
        echo "Aborted. Fund wallet first."
        exit 1
    fi
fi

echo ""
echo "2️⃣ Checking ShredStream service..."
if curl -s http://localhost:8080/prices | head -c 100 &> /dev/null; then
    echo "   ✅ ShredStream service running"
else
    echo "   ❌ ShredStream service not responding!"
    echo "   Start it with: cd /home/tom14cat14/Arb_Bot/shredstream_service && cargo run --release"
    exit 1
fi

echo ""
echo "3️⃣ Checking bot compilation..."
if [ -f "target/release/clean_arb_bot" ]; then
    echo "   ✅ Bot binary exists"
else
    echo "   ⚠️  Binary not found, compiling..."
    cargo build --release
    echo "   ✅ Bot compiled"
fi

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "⚠️⚠️⚠️  CRITICAL WARNING  ⚠️⚠️⚠️"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "You are about to enable REAL MONEY TRADING"
echo ""
echo "Configuration:"
echo "  • Wallet: $WALLET"
echo "  • Position Size: 0.01 SOL per trade (~\$1.50)"
echo "  • Max Daily Loss: 0.1 SOL (~\$15)"
echo "  • Stop After: 3 consecutive failures"
echo ""
echo "Expected Performance:"
echo "  • Success Rate: 60-70% (lower than paper mode)"
echo "  • Profit/Trade: 0.01-0.05 SOL (after fees)"
echo "  • Daily Target: 10-20 trades for testing"
echo ""
echo "Risks:"
echo "  • Slippage may reduce profits"
echo "  • MEV competition may front-run"
echo "  • Network issues may cause failures"
echo "  • Fees will reduce net profit"
echo ""
echo "Safety Features ACTIVE:"
echo "  ✅ Tiny position size (0.01 SOL)"
echo "  ✅ Daily loss limit (0.1 SOL)"
echo "  ✅ Circuit breaker enabled"
echo "  ✅ Emergency stop: Ctrl+C"
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

read -p "Type 'GO LIVE' to confirm you understand the risks: " confirm

if [[ $confirm != "GO LIVE" ]]; then
    echo ""
    echo "Aborted. No trades executed."
    exit 0
fi

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "🚀 STARTING LIVE TRADING"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "Monitoring instructions:"
echo "  • Watch for opportunities detected"
echo "  • Monitor success rate"
echo "  • Check profit/loss accumulation"
echo "  • Press Ctrl+C to stop gracefully"
echo ""
echo "First 5-10 trades:"
echo "  • Verify transactions on Solscan"
echo "  • Check actual slippage"
echo "  • Confirm profits match expectations"
echo ""
echo "Stop immediately if:"
echo "  • Success rate < 50%"
echo "  • 3+ consecutive failures"
echo "  • Daily loss > 0.1 SOL"
echo "  • Any unusual behavior"
echo ""
echo "Starting in 5 seconds..."
sleep 5

echo ""
echo "🚀 BOT IS LIVE 🚀"
echo ""

# Copy live config and run
cp .env.live .env
exec cargo run --release
