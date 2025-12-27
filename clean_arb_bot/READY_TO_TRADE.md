# Arb Bot - Ready to Trade (Real Money Config)

**Date**: 2025-11-07
**Status**: ✅ CONFIGURED FOR REAL MONEY - Ready for Paper Testing

---

## ✅ CURRENT STATUS

### Infrastructure Ready
- ✅ **Wallet**: 1.9710 SOL (`9WrFdecsvMogYEtwjGrCBs4LrfnZhm9QKigD4CdcD3kA`)
- ✅ **Arb Bot**: Compiled (11MB binary at `target/release/clean_arb_bot`)
- ✅ **ShredStream Service**: Running on port 8080 with **797 prices** cached
- ✅ **JITO Integration**: Working (base58 encoding, dynamic tipping, rate limiting)

### Configuration (Production Settings)
```
Capital: 1.0 SOL tradable (0.9 SOL per trade)
Min Profit Margin: 1.0x fees
Max Daily Trades: 200
Max Consecutive Failures: 10
Daily Loss Limit: 0.2 SOL
JITO: 99th percentile (up to 0.005 SOL max)
```

**Configuration file**: `.env` (already set to production values)

---

## 🚀 QUICK START - PAPER TRADING FIRST

### Step 1: Switch to Paper Mode (Keep Real Money Config)

```bash
cd /home/tom14cat14/Arb_Bot/clean_arb_bot

# Edit .env - only change these 2 lines:
nano .env
```

**Change ONLY these lines**:
```bash
ENABLE_REAL_TRADING=true       # Keep this TRUE (uses real trading logic)
PAPER_TRADING=true             # Change to TRUE for paper testing
```

All other settings stay the same (position sizes, fees, limits, etc.)

Save and exit (`Ctrl+X`, `Y`, `Enter`)

---

### Step 2: Start Paper Trading (3 Terminals)

**Terminal 1: ShredStream Service** (Already running ✅)
```bash
cd /home/tom14cat14/shared/shredstream_service
# Already started - check health:
curl http://localhost:8080/health
# Should show: "prices_cached": 797+
```

**Terminal 2: Paper Trading Bot**
```bash
cd /home/tom14cat14/Arb_Bot/clean_arb_bot

# Start paper trading
env RUST_LOG=info ./target/release/clean_arb_bot \
  | tee paper_test_$(date +%Y%m%d_%H%M%S).log
```

**Terminal 3: Monitor**
```bash
cd /home/tom14cat14/Arb_Bot/clean_arb_bot

# Watch for key events
tail -f paper_test_*.log | grep -E "Opportunity|Trade|Profit|Error|Warning"
```

---

### Step 3: Paper Testing Validation (30-60 minutes)

Watch for:
- ✅ **Opportunities detected** (should see some within 5-10 minutes)
- ✅ **Profit calculations** (check if reasonable)
- ✅ **Price data flowing** (from ShredStream)
- ✅ **No crashes or panics**
- ✅ **Safety limits trigger correctly** (if thresholds hit)

**Sample good output**:
```
[INFO] Fetched 797 prices from ShredStream
[INFO] Opportunity detected: Token ABC, spread 2.5%, estimated profit 0.015 SOL
[INFO] Filters passed: Volume ✓, Liquidity ✓, Margin ✓
[INFO] [PAPER MODE] Would execute: Buy 0.9 SOL of ABC
[INFO] [PAPER MODE] Estimated net profit: +0.012 SOL
```

**Sample rejection (normal)**:
```
[INFO] Opportunity detected: Token XYZ, spread 1.2%
[WARN] Rejected: Spread too low (below min after fees)
```

---

### Step 4: After Paper Trading Success

**Review results**:
```bash
cd /home/tom14cat14/Arb_Bot/clean_arb_bot

# Count opportunities
grep -c "Opportunity detected" paper_test_*.log

# Count would-be trades
grep -c "Would execute" paper_test_*.log

# Check for errors
grep -i "error\|panic\|crash" paper_test_*.log

# Check profitability
grep "profit:" paper_test_*.log | tail -20
```

**If paper trading looks good**:
1. Stop the bot (Ctrl+C in Terminal 2)
2. Proceed to "GO LIVE" section below

**If paper trading shows issues**:
1. Stop the bot (Ctrl+C in Terminal 2)
2. Share the log file with me for analysis
3. Fix issues before going live

---

## 💰 GO LIVE (After Paper Trading Success)

### Step 1: Switch to Live Mode

```bash
cd /home/tom14cat14/Arb_Bot/clean_arb_bot

# Edit .env - only change this 1 line:
nano .env
```

**Change ONLY this line**:
```bash
PAPER_TRADING=false            # Change to FALSE for real money
```

All other settings stay the same. Save and exit.

---

### Step 2: Verify Wallet Balance

```bash
# Check balance one more time
curl -s https://api.mainnet-beta.solana.com -X POST -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"getBalance","params":["9WrFdecsvMogYEtwjGrCBs4LrfnZhm9QKigD4CdcD3kA"]}' \
  | python3 -c "import sys, json; data = json.load(sys.stdin); print(f'{data[\"result\"][\"value\"]/1e9:.4f} SOL')"

# Should show: ~1.97 SOL
```

---

### Step 3: Start Live Trading

**Terminal 1: ShredStream Service** (Already running ✅)

**Terminal 2: Live Trading**
```bash
cd /home/tom14cat14/Arb_Bot/clean_arb_bot

# Start live trading with REAL MONEY ⚠️
env RUST_LOG=info ./target/release/clean_arb_bot \
  | tee live_trading_$(date +%Y%m%d_%H%M%S).log
```

**Terminal 3: Monitor CLOSELY**
```bash
cd /home/tom14cat14/Arb_Bot/clean_arb_bot

# Watch every trade
tail -f live_trading_*.log | grep -E "Opportunity|Executing|Profit|Error|Success|Failed"
```

---

### Step 4: Monitor First 5-10 Trades VERY CLOSELY

After each trade:
```bash
# Check wallet balance
curl -s https://api.mainnet-beta.solana.com -X POST -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"getBalance","params":["9WrFdecsvMogYEtwjGrCBs4LrfnZhm9QKigD4CdcD3kA"]}' \
  | python3 -c "import sys, json; data = json.load(sys.stdin); print(f'{data[\"result\"][\"value\"]/1e9:.4f} SOL')"

# Verify on Solscan
# Go to: https://solscan.io/account/9WrFdecsvMogYEtwjGrCBs4LrfnZhm9QKigD4CdcD3kA
```

---

## 🛑 EMERGENCY STOP

**If anything looks wrong** (any terminal):
```bash
# Stop bot immediately
pkill -9 clean_arb_bot

# Verify stopped
ps aux | grep clean_arb_bot

# Check wallet balance
curl -s https://api.mainnet-beta.solana.com -X POST -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"getBalance","params":["9WrFdecsvMogYEtwjGrCBs4LrfnZhm9QKigD4CdcD3kA"]}' \
  | python3 -c "import sys, json; data = json.load(sys.stdin); print(f'{data[\"result\"][\"value\"]/1e9:.4f} SOL')"
```

---

## 🚨 STOP IMMEDIATELY IF:

### During Paper Trading:
- ❌ No opportunities after 30 minutes
- ❌ Profit calculations look wrong (negative, unrealistic)
- ❌ Any crashes or panics
- ❌ ShredStream connection issues

### During Live Trading:
- ❌ 3+ consecutive failures
- ❌ Wallet balance dropping unexpectedly
- ❌ Fees higher than expected (>0.01 SOL per trade)
- ❌ Any trade loses money
- ❌ Balance below 1.0 SOL

---

## 📊 QUICK REFERENCE

### Check ShredStream Status
```bash
curl http://localhost:8080/health | python3 -m json.tool
```

### Check Wallet Balance
```bash
curl -s https://api.mainnet-beta.solana.com -X POST -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"getBalance","params":["9WrFdecsvMogYEtwjGrCBs4LrfnZhm9QKigD4CdcD3kA"]}' \
  | python3 -c "import sys, json; data = json.load(sys.stdin); print(f'{data[\"result\"][\"value\"]/1e9:.4f} SOL')"
```

### View Recent Trades
```bash
cd /home/tom14cat14/Arb_Bot/clean_arb_bot
tail -50 live_trading_*.log | grep "Profit\|Trade\|Success\|Failed"
```

### Emergency Kill
```bash
pkill -9 clean_arb_bot
```

---

## ✅ READY TO START?

**Current Status**:
1. ✅ ShredStream Service running (797 prices)
2. ✅ Arb Bot compiled
3. ✅ Wallet funded (1.9710 SOL)
4. ✅ Configuration ready (production settings)

**Next Steps**:
1. **NOW**: Set `PAPER_TRADING=true` in `.env`
2. **NOW**: Run paper trading for 30-60 minutes (Terminal 2)
3. **AFTER SUCCESS**: Set `PAPER_TRADING=false` in `.env`
4. **THEN**: Start live trading

**Let's start with paper trading!** 🎯

---

**File Locations**:
- Config: `/home/tom14cat14/Arb_Bot/clean_arb_bot/.env`
- Binary: `/home/tom14cat14/Arb_Bot/clean_arb_bot/target/release/clean_arb_bot`
- Logs: `/home/tom14cat14/Arb_Bot/clean_arb_bot/paper_test_*.log` (paper)
- Logs: `/home/tom14cat14/Arb_Bot/clean_arb_bot/live_trading_*.log` (live)
