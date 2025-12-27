#!/bin/bash

echo "🔍 Testing Arb Bot UDP ShredStream Listener (Port 20000)"
echo "=========================================================="
echo ""

# Check firewall
echo "1️⃣ Checking firewall status..."
sudo ufw status | grep 20000 || echo "⚠️  Port 20000 not in firewall rules"
echo ""

# Open port if needed
read -p "Open UDP port 20000 in firewall? (y/n) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    sudo ufw allow 20000/udp
    echo "✅ Port 20000/UDP opened"
fi
echo ""

# Check if port is already in use
echo "2️⃣ Checking if port 20000 is already in use..."
sudo netstat -tuln | grep :20000 && echo "⚠️  Port already in use" || echo "✅ Port 20000 available"
echo ""

# Start bot in background
echo "3️⃣ Starting Arb Bot UDP listener..."
cd /home/tom14cat14/Arb_Bot
env PAPER_TRADING=true RUST_LOG=info timeout 60 ./target/release/arb_bot > /tmp/arb_udp_test.log 2>&1 &
BOT_PID=$!
echo "✅ Bot started (PID: $BOT_PID)"
echo ""

# Wait for socket to bind
sleep 2

# Check if listening
echo "4️⃣ Verifying UDP socket binding..."
sudo netstat -tuln | grep :20000 && echo "✅ Bot listening on port 20000" || echo "❌ Bot NOT listening"
echo ""

# Monitor with tcpdump for 10 seconds
echo "5️⃣ Monitoring for UDP packets (10 seconds)..."
echo "   (This will show if ERPC is pushing shreds to your IP)"
echo ""
sudo timeout 10 tcpdump -i any udp port 20000 -c 5 2>&1 | grep -E "(listening|IP)" || echo "No packets received in 10 seconds"
echo ""

# Check bot logs
echo "6️⃣ Bot log output:"
echo "────────────────────────────────────────────────────────"
tail -30 /tmp/arb_udp_test.log
echo "────────────────────────────────────────────────────────"
echo ""

# Stop bot
echo "7️⃣ Stopping bot..."
kill $BOT_PID 2>/dev/null
wait $BOT_PID 2>/dev/null
echo "✅ Bot stopped"
echo ""

# Analysis
echo "📊 TEST RESULTS ANALYSIS"
echo "=========================================================="

if sudo netstat -tuln | grep -q :20000; then
    echo "✅ UDP socket bound successfully"
else
    echo "❌ UDP socket binding failed"
fi

if grep -q "ShredStream UDP socket bound" /tmp/arb_udp_test.log; then
    echo "✅ Bot initialized UDP listener correctly"
else
    echo "❌ Bot failed to initialize UDP listener"
fi

if grep -q "Received UDP packet" /tmp/arb_udp_test.log; then
    echo "✅ UDP PACKETS RECEIVED! ShredStream is working!"
    PACKET_COUNT=$(grep -c "Received UDP packet" /tmp/arb_udp_test.log)
    echo "   → Received $PACKET_COUNT packets in 60 seconds"
else
    echo "⚠️  No UDP packets received"
    echo ""
    echo "Possible causes:"
    echo "  1. IP whitelist not active yet (contact ERPC support)"
    echo "  2. Firewall blocking inbound UDP (check cloud provider)"
    echo "  3. NAT/PAT routing issue (try dedicated IP)"
    echo "  4. Wrong port number (confirm with ERPC it's port 20000)"
fi

echo ""
echo "Full log saved to: /tmp/arb_udp_test.log"
echo "=========================================================="
