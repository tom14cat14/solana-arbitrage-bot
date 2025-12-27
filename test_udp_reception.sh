#!/bin/bash
# Test UDP ShredStream Reception
# Tests if ERPC is pushing shreds to port 20000

echo "════════════════════════════════════════════════════════════════"
echo "🔍 UDP ShredStream Reception Test"
echo "════════════════════════════════════════════════════════════════"
echo ""

# Check if running as root (needed for tcpdump)
if [ "$EUID" -ne 0 ]; then
    echo "⚠️  This script needs sudo for tcpdump"
    echo "    Run: sudo ./test_udp_reception.sh"
    exit 1
fi

echo "📝 Test Configuration:"
echo "   • Port: 20000/UDP"
echo "   • IP: 151.243.244.130 (whitelisted)"
echo "   • Protocol: Raw UDP shred forwarding"
echo "   • Direction: INBOUND (ERPC → YOU)"
echo ""

# Test 1: Check firewall
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Test 1: Firewall Configuration"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

if command -v ufw &> /dev/null; then
    echo "Checking UFW rules..."
    ufw status | grep 20000 || echo "❌ Port 20000/UDP not allowed in UFW"
    echo ""
    echo "To fix: sudo ufw allow 20000/udp"
else
    echo "✅ UFW not installed (firewall may be disabled)"
fi
echo ""

# Test 2: Check if port is listening
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Test 2: Port Binding Status"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

echo "Checking if anything is listening on port 20000/UDP..."
netstat -ulnp | grep 20000 || echo "❌ Nothing listening on port 20000/UDP"
echo ""
echo "To start bot: cd /home/tom14cat14/Arb_Bot && \\"
echo "              env PAPER_TRADING=true ENABLE_UDP_LISTENER=true \\"
echo "              RUST_LOG=info ~/.cargo/bin/cargo run --release"
echo ""

# Test 3: Monitor for incoming UDP packets
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Test 3: Monitor for Incoming UDP Packets (30 seconds)"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "Listening for ANY UDP traffic on port 20000..."
echo "Press Ctrl+C to stop early"
echo ""

timeout 30 tcpdump -i any udp port 20000 -v -n -c 10 2>&1 || {
    echo ""
    echo "❌ No UDP packets received on port 20000 in 30 seconds"
    echo ""
    echo "Possible causes:"
    echo "  1. IP whitelist (151.243.244.130) not activated by ERPC"
    echo "  2. Wrong port (may not be 20000)"
    echo "  3. ERPC not configured to forward to this IP"
    echo "  4. Network routing issue (NAT/PAT)"
    echo ""
    echo "Next steps:"
    echo "  • Contact ERPC support (Validators DAO Discord)"
    echo "  • Verify IP whitelist is active"
    echo "  • Confirm correct UDP port for ShredStream"
    echo "  • Ask if shreds are being forwarded"
}

echo ""
echo "════════════════════════════════════════════════════════════════"
echo "📊 Test Complete"
echo "════════════════════════════════════════════════════════════════"
echo ""

# Test 4: Check network routes
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Test 4: Network Route to ERPC"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

echo "Testing connectivity to grpc-ny6-1.erpc.global..."
ping -c 3 grpc-ny6-1.erpc.global || echo "❌ Cannot reach ERPC endpoint"
echo ""

echo "Checking public IP (should match 151.243.244.130)..."
curl -s https://api.ipify.org || echo "❌ Cannot determine public IP"
echo ""
echo ""

# Test 5: DNS resolution
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Test 5: DNS Resolution"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

echo "Resolving grpc-ny6-1.erpc.global..."
nslookup grpc-ny6-1.erpc.global || echo "❌ DNS resolution failed"
echo ""

echo "════════════════════════════════════════════════════════════════"
echo "💡 Summary"
echo "════════════════════════════════════════════════════════════════"
echo ""
echo "If no UDP packets were received:"
echo "  1. ✅ Bot code is correct (INBOUND listener on port 20000)"
echo "  2. ⏳ ERPC may not be pushing shreds yet"
echo "  3. 📞 Contact ERPC support to activate IP whitelist"
echo ""
echo "If UDP packets WERE received:"
echo "  1. ✅ ERPC is pushing shreds!"
echo "  2. ✅ Next step: Implement shred decoding"
echo "  3. 📊 Run bot and watch for price updates"
echo ""
echo "════════════════════════════════════════════════════════════════"
