# Live Trading Guide - Real Money Trading

**Created**: 2025-10-08
**Wallet**: `9WrFdecsvMogYEtwjGrCBs4LrfnZhm9QKigD4CdcD3kA`
**Status**: Ready for live trading with monitoring

---

## 🚀 **HOW TO START LIVE TRADING**

### **Step 1: Start ShredStream Service** (Terminal 1)

```bash
cd /home/tom14cat14/Arb_Bot/shredstream_service
~/.cargo/bin/cargo run --release
```

Wait for: `✅ Connected to ShredStream successfully`

---

### **Step 2: Start Live Trading Bot** (Terminal 2)

```bash
cd /home/tom14cat14/Arb_Bot/clean_arb_bot
./start_live_trading.sh
```

This will:
- ✅ Check wallet balance
- ✅ Verify ShredStream is running
- ✅ Show current configuration
- ⏰ Give you 10 seconds to abort
- 📝 Start trading with full logging

---

### **Step 3: Monitor Trading** (Terminal 3)

```bash
cd /home/tom14cat14/Arb_Bot/clean_arb_bot
./monitor_live_trades.sh
```

This shows real-time filtered output:
- 💰 Opportunities detected
- 📦 Trades submitted
- ✅ Successful executions
- ❌ Failed trades
- ⚠️ Warnings and errors

---

## 📊 **MONITORING COMMANDS**

### **Check Current Status**
```bash
cd /home/tom14cat14/Arb_Bot/clean_arb_bot
./check_trading_status.sh
```

Shows:
- Bot running status
- Current wallet balance
- Trade statistics
- Recent events

### **Check Wallet Balance**
```bash
solana balance 9WrFdecsvMogYEtwjGrCBs4LrfnZhm9QKigD4CdcD3kA
```

### **View Full Logs**
```bash
cd /home/tom14cat14/Arb_Bot/clean_arb_bot/logs
tail -f live_trading_*.log
```

### **Count Trades**
```bash
cd /home/tom14cat14/Arb_Bot/clean_arb_bot/logs
LATEST_LOG=$(ls -t live_trading_*.log | head -1)

echo "Opportunities: $(grep -c 'Opportunity detected' $LATEST_LOG)"
echo "Submitted: $(grep -c 'Submitting' $LATEST_LOG)"
echo "Successful: $(grep -c 'executed successfully' $LATEST_LOG)"
echo "Failed: $(grep -c 'failed' $LATEST_LOG)"
```

---

## 🛑 **HOW TO STOP TRADING**

### **Method 1: Ctrl+C**
In the terminal running the bot, press `Ctrl+C`

### **Method 2: Killswitch**
```bash
cd /home/tom14cat14/Arb_Bot/clean_arb_bot
touch .killswitch
```

### **Method 3: Kill Process**
```bash
pkill -9 -f clean_arb_bot
```

---

## 📋 **CURRENT CONFIGURATION**

From your `.env` file:

```
Wallet: 9WrFdecsvMogYEtwjGrCBs4LrfnZhm9QKigD4CdcD3kA
Total Balance: 1.0 SOL
Tradable Capital: 0.9 SOL
Fee Reserve: 0.1 SOL

Position Size: 0.9 SOL per trade (FULL CAPITAL)
Min Profit Margin: fees + 0.5% of gross profit (realistic arbitrage)
Min Spread: 0.5%

Max Daily Trades: 50
Max Consecutive Failures: 3
Daily Loss Limit: 0.1 SOL

JITO Bundles: Disabled (direct RPC)
```

---

## ⚠️ **SAFETY LIMITS (Automatic)**

The bot will automatically stop if:

1. **3 consecutive failures** → Circuit breaker trips
2. **Daily loss ≥ 0.1 SOL** → Daily loss limit reached
3. **50 trades in one day** → Daily trade limit reached
4. **Balance < 0.1 SOL** → Insufficient funds for fees

---

## 🎯 **WHAT I'LL MONITOR FOR YOU**

I can help you monitor by analyzing the logs. Just ask me to:

1. **"Check trading status"** → I'll analyze current session
2. **"Show recent trades"** → I'll show last 10 trades
3. **"Calculate P&L"** → I'll sum up profits/losses
4. **"Check for errors"** → I'll highlight any issues
5. **"Show statistics"** → Win rate, success rate, etc.

---

## 📊 **EXPECTED BEHAVIOR**

### **First 10 Minutes**
- Bot connects to ShredStream
- Starts receiving price data
- May detect 0-10 opportunities
- Most opportunities rejected by filters

### **Normal Operation**
```
⚡ Fetched 514 prices in 1.3ms
💰 Opportunity detected: Token ABC, spread 2.3%, profit 0.015 SOL
✅ Filters passed: Volume ✓, Liquidity ✓, Margin ✓
📦 Submitting trade: 0.9 SOL position
✅ Trade executed successfully: signature abc123...
💰 Profit: +0.012 SOL (after fees)
```

### **Rejection Example**
```
💰 Opportunity detected: Token XYZ, spread 5.2%, profit 0.045 SOL
❌ Rejected: Volume too low (4,899 SOL < 10,000 SOL required)
```

---

## 🔍 **LOG FILE LOCATIONS**

All logs saved to:
```
/home/tom14cat14/Arb_Bot/clean_arb_bot/logs/
```

Files:
- `live_trading_YYYYMMDD_HHMMSS.log` - Full trading session logs

---

## 💡 **RECOMMENDED MONITORING WORKFLOW**

### **Setup (3 terminals)**

**Terminal 1**: ShredStream Service
```bash
cd /home/tom14cat14/Arb_Bot/shredstream_service
~/.cargo/bin/cargo run --release
```

**Terminal 2**: Live Trading Bot
```bash
cd /home/tom14cat14/Arb_Bot/clean_arb_bot
./start_live_trading.sh
```

**Terminal 3**: Monitoring
```bash
cd /home/tom14cat14/Arb_Bot/clean_arb_bot
./monitor_live_trades.sh
```

### **Check Status Every 5 Minutes**
```bash
cd /home/tom14cat14/Arb_Bot/clean_arb_bot
./check_trading_status.sh
```

### **Share Logs with Me**
```bash
# Get last 50 lines of log
cd /home/tom14cat14/Arb_Bot/clean_arb_bot/logs
tail -50 live_trading_*.log > /tmp/recent_activity.txt
```

Then paste the contents and I'll analyze it for you.

---

## 🚨 **EMERGENCY PROCEDURES**

### **If Something Looks Wrong**

1. **Stop the bot immediately**: `Ctrl+C` or `touch .killswitch`
2. **Check wallet balance**: `solana balance 9WrFdecsvMogYEtwjGrCBs4LrfnZhm9QKigD4CdcD3kA`
3. **Share logs with me**: Last 100 lines from the log file
4. **I'll analyze** what happened and advise next steps

### **Warning Signs to Watch For**

- ❌ Many consecutive failures (>3)
- ❌ Balance dropping quickly
- ❌ Repeated errors in logs
- ❌ Unusually high transaction fees
- ❌ No successful trades after 20 attempts

---

## ✅ **READY TO START?**

1. Open 3 terminals
2. Start ShredStream service (Terminal 1)
3. Start live trading bot (Terminal 2)
4. Start monitoring (Terminal 3)
5. Ask me to check status periodically
6. Share logs if anything looks unusual

**I'll be monitoring with you!** 🎯
