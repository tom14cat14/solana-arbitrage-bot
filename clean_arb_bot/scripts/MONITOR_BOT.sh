#!/bin/bash
# Comprehensive Bot Monitoring Script

echo "=========================================="
echo "🤖 ARB BOT MONITORING - REAL TIME"
echo "=========================================="
echo ""

# Check if processes are running
echo "📊 Process Status:"
echo "----------------------------------------"
if pgrep -f "clean_arb_bot" > /dev/null; then
    PID=$(pgrep -f "clean_arb_bot")
    echo "✅ Arb Bot: RUNNING (PID: $PID)"
else
    echo "❌ Arb Bot: NOT RUNNING"
fi

if pgrep -f "shredstream_service" > /dev/null; then
    PID=$(pgrep -f "shredstream_service")
    echo "✅ ShredStream: RUNNING (PID: $PID)"
else
    echo "❌ ShredStream: NOT RUNNING"
fi
echo ""

# Check wallet balance
echo "💰 Wallet Balance:"
echo "----------------------------------------"
solana balance 9WrFdecsvMogYEtwjGrCBs4LrfnZhm9QKigD4CdcD3kA 2>/dev/null || echo "Unable to check balance"
echo ""

# Show recent log activity
echo "📝 Recent Activity (last 20 lines):"
echo "----------------------------------------"
tail -20 /tmp/arb_bot_live.log | grep -E "(SECURE|Opportunity|Submitting|executed|failed|ERROR|WARN)" || echo "No recent activity"
echo ""

# Trade statistics
echo "📊 Trade Statistics:"
echo "----------------------------------------"
echo "Opportunities detected: $(grep -c "Opportunity detected" /tmp/arb_bot_live.log 2>/dev/null || echo 0)"
echo "Bundles submitted: $(grep -c "Submitting" /tmp/arb_bot_live.log 2>/dev/null || echo 0)"
echo "Successful trades: $(grep -c "executed successfully" /tmp/arb_bot_live.log 2>/dev/null || echo 0)"
echo "Failed trades: $(grep -c "FAILED" /tmp/arb_bot_live.log 2>/dev/null || echo 0)"
echo "Errors: $(grep -c "ERROR" /tmp/arb_bot_live.log 2>/dev/null || echo 0)"
echo "Warnings: $(grep -c "WARN" /tmp/arb_bot_live.log 2>/dev/null || echo 0)"
echo ""

# Security check
echo "🔒 Security Status:"
echo "----------------------------------------"
SECURE_COUNT=$(grep -c "SECURE: JITO tip" /tmp/arb_bot_live.log 2>/dev/null || echo 0)
OLD_WARNING=$(grep -c "Current implementation adds tip as separate tx" /tmp/arb_bot_live.log 2>/dev/null || echo 0)

if [ "$SECURE_COUNT" -gt 0 ]; then
    echo "✅ Using SECURE method: $SECURE_COUNT transactions with tip INSIDE"
fi

if [ "$OLD_WARNING" -gt 0 ]; then
    echo "⚠️ WARNING: Old insecure method detected ($OLD_WARNING times) - restart bot!"
else
    echo "✅ No security warnings (fix working!)"
fi
echo ""

echo "=========================================="
echo "Commands:"
echo "  Watch live: tail -f /tmp/arb_bot_live.log | grep -E '(Opportunity|Submitting|executed|SECURE)'"
echo "  Stop bot: pkill -9 -f clean_arb_bot"
echo "  Check balance: solana balance 9WrFdecsvMogYEtwjGrCBs4LrfnZhm9QKigD4CdcD3kA"
echo "=========================================="
