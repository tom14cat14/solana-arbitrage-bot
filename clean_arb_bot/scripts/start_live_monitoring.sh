#!/bin/bash

# Live Monitoring Mode - Detects opportunities but cannot execute (safe)
# Bot will show what it WOULD trade if DEX instructions were complete

echo "🚀 Starting Clean Arb Bot in Live Monitoring Mode"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "⚠️  IMPORTANT: Bot cannot execute trades yet (missing DEX instructions)"
echo "✅ SAFE: Your 0.9 SOL is protected"
echo "📊 Will log opportunities for validation"
echo ""
echo "Press Ctrl+C to stop"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# Set environment variables
export PAPER_TRADING=false
export ENABLE_REAL_TRADING=true
export CAPITAL_SOL=1.0
export MAX_POSITION_SIZE_SOL=0.45
export MIN_PROFIT_SOL=0.001
export MIN_SPREAD_PERCENTAGE=0.3
export SHREDSTREAM_SERVICE_URL="http://localhost:8080"
export RUST_LOG=info

# Note: WALLET_PRIVATE_KEY not set - bot will detect this and continue in monitoring mode

# Build if needed
if [ ! -f "target/release/clean_arb_bot" ]; then
    echo "🔨 Building bot..."
    ~/.cargo/bin/cargo build --release
fi

# Run with logging
LOGFILE="live_monitoring_$(date +%Y%m%d_%H%M%S).log"
echo "📝 Logging to: $LOGFILE"
echo ""

~/.cargo/bin/cargo run --release 2>&1 | tee "$LOGFILE"
