#!/bin/bash

echo "🔍 Testing Arb Bot with Real ERPC ShredStream Data"
echo "=================================================="
echo ""

# Configuration
export SHREDS_ENDPOINT="https://shreds-ny6-1.erpc.global"
export RUST_LOG="info,arb_bot::real_shredstream=debug,arb_bot::protobuf_processor=debug,arb_bot::dex_transaction_parser=debug"
export PAPER_TRADING="true"
export ENABLE_REAL_TRADING="false"

echo "📡 Configuration:"
echo "  • ShredStream: $SHREDS_ENDPOINT"
echo "  • Paper Trading: Enabled"
echo "  • Duration: 60 seconds"
echo ""

echo "✅ What we're testing:"
echo "  1. UDP connection to ERPC ShredStream"
echo "  2. Real blockchain shred reception"
echo "  3. DEX swap instruction parsing"
echo "  4. Real price extraction"
echo "  5. Arbitrage detection with real prices"
echo ""

echo "🚫 What should NOT happen:"
echo "  • No mock/fake price generation"
echo "  • No simulated data fallback"
echo "  • No random price variations"
echo ""

echo "▶️  Starting bot (60 second test)..."
echo "=================================================="
echo ""

# Run for 60 seconds and capture output
timeout 60 ./target/release/arb_bot 2>&1 | tee /tmp/arb_bot_test.log

echo ""
echo "=================================================="
echo "📊 Test Results Analysis"
echo "=================================================="
echo ""

# Check for indicators of real data
echo "✅ Checking for real data indicators..."
echo ""

if grep -q "UDP.*ShredStream" /tmp/arb_bot_test.log; then
    echo "  ✓ UDP ShredStream connection mentioned"
else
    echo "  ✗ No UDP connection detected"
fi

if grep -q "REAL.*swap\|REAL.*price" /tmp/arb_bot_test.log; then
    echo "  ✓ Real swap/price data detected"
else
    echo "  ✗ No real swap data found"
fi

if grep -q "Extracted.*prices.*blockchain" /tmp/arb_bot_test.log; then
    echo "  ✓ Price extraction from blockchain confirmed"
else
    echo "  ✗ No blockchain price extraction"
fi

echo ""
echo "🚫 Checking for fake data indicators..."
echo ""

if grep -qi "mock.*price\|fake.*price\|simulated.*price" /tmp/arb_bot_test.log; then
    echo "  ✗ WARNING: Mock/fake price data detected!"
else
    echo "  ✓ No mock price data found"
fi

if grep -qi "random.*variation\|fastrand" /tmp/arb_bot_test.log; then
    echo "  ✗ WARNING: Random price generation detected!"
else
    echo "  ✓ No random price generation"
fi

if grep -qi "fallback.*mock" /tmp/arb_bot_test.log; then
    echo "  ✗ WARNING: Mock data fallback used!"
else
    echo "  ✓ No mock fallback triggered"
fi

echo ""
echo "📈 Arbitrage Detection..."
echo ""

ARB_COUNT=$(grep -c "Arbitrage opportunity" /tmp/arb_bot_test.log || echo "0")
echo "  • Opportunities detected: $ARB_COUNT"

if [ "$ARB_COUNT" -gt 0 ]; then
    echo "  ✓ Arbitrage detection working"
    echo ""
    echo "Sample opportunities:"
    grep "Arbitrage opportunity" /tmp/arb_bot_test.log | head -3
else
    echo "  ℹ No opportunities detected (may be normal if spreads are small)"
fi

echo ""
echo "=================================================="
echo "Full log saved to: /tmp/arb_bot_test.log"
echo "Review with: less /tmp/arb_bot_test.log"
echo "=================================================="
