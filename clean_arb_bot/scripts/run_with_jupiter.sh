#!/bin/bash
# Run arbitrage bot with Jupiter triangle detection

killall clean_arb_bot 2>/dev/null || true
sleep 1

echo "🚀 Starting Arbitrage Bot with Jupiter Triangle Detection..."
echo ""

env \
  JUPITER_API_KEY="2b58d214-0f97-45a4-b969-548f6137d188" \
  MIN_PROFIT_SOL=0.001 \
  MIN_SPREAD_PERCENTAGE=0.05 \
  PAPER_TRADING=true \
  RUST_LOG=info \
  ./target/release/clean_arb_bot

