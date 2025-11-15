# Monitoring Guide - Zero Performance Impact
**Created:** 2025-10-11
**Purpose:** Watch bot performance without slowing it down

---

## 🎯 Your Question Answered

**Q: How can I watch how close we are to profit margin without slowing the bot down?**

**A: Use the monitoring scripts!** They read the log file separately from the bot, so **ZERO performance impact**.

---

## 📊 Two Monitoring Options

### Option 1: Live Dashboard (Recommended)

**Full visual dashboard with real-time metrics:**

```bash
./monitor_dashboard.sh
```

**Shows:**
- 🎯 **How close to execution** (your key metric!)
- Opportunities detected
- Simulation pass rates
- Recent profitable opportunities
- Critical error count (should be 0)
- Latest opportunity details
- Market volatility analysis

**Updates:** Every 2 seconds
**Performance Impact:** ZERO (reads logs only)
**Exit:** Ctrl+C

---

### Option 2: Quick One-Line Status

**Fast single-line summary:**

```bash
./quick_status.sh
```

**Example Output:**
```
[🟡 FINDING OPPS] Opps:4 | PassRate:0% | JITO:0 | LastProfit:0.009513 SOL | Errors(101):0
```

**Perfect for:**
- Quick checks
- Adding to cron jobs
- Monitoring scripts
- SSH into server for quick peek

---

## 🔑 Key Metrics Explained

### 1. **Execution Success Rate (How Close to Profit)**

**This is THE metric you asked about!**

```
🎯 Execution Success Rate: 25.0%
```

**What it means:**
- **0%**: High volatility, opportunities going stale quickly
- **1-20%**: Finding opportunities, waiting for stability
- **21-50%**: Getting closer, some stable windows
- **51-80%**: Very close! Market conditions improving
- **>80%**: Excellent conditions, trades should land soon

**Formula:**
```
Success Rate = (Passed Initial Sim - Failed Final Sim) / Passed Initial Sim
```

**Why it matters:**
- Shows how close opportunities are to actual execution
- Indicates market stability
- Predicts when trades will land

---

### 2. **Opportunities Detected**

```
🔍 Opportunities Detected: 25
```

**What it means:**
- Bot is finding arbitrage opportunities
- Higher = more active market
- Should be > 0 continuously

**Good:** 10+ per minute
**Normal:** 5-10 per minute
**Concerning:** < 1 per minute (check ShredStream)

---

### 3. **Initial Simulations Passed**

```
✅ Initial Simulations Passed: 20
```

**What it means:**
- Opportunities that passed first validation
- Instructions built correctly
- Should be close to "Opportunities Detected"

**Good:** 90%+ of opportunities
**Indicates:** All DEX fixes working correctly

---

### 4. **Final Simulations Failed**

```
⏳ Final Simulations Failed: 18 (market volatility)
```

**What it means:**
- Opportunities that became stale before execution
- Pool state changed in the 40-50ms window
- This is NORMAL and protects your capital

**Normal:** 50-95% in volatile markets
**Why it's good:** Prevents wasting JITO submission costs

---

### 5. **JITO Bundles Submitted**

```
🚀 JITO Bundles Submitted: 3 (TOTAL ALL TIME)
```

**What it means:**
- Actual trades sent to blockchain
- These are REAL money trades
- Check wallet after seeing this!

**Each submission:**
- View on Solscan
- Check transaction signature
- Verify profit/loss

---

### 6. **Custom(101) Errors (CRITICAL)**

```
✅ Custom(101) Errors: 0 (PERFECT)
```

**MUST BE ZERO!**

- **0**: All DEX fixes working ✅
- **>0**: Code regression, STOP BOT immediately ❌

**If not zero:**
```bash
# Stop bot immediately
tmux kill-session -t arb_bot

# Report issue
echo "Custom(101) errors detected at $(date)" >> CRITICAL_ERRORS.log
```

---

## 📺 Dashboard Visual Example

```
═══════════════════════════════════════════════════════════════
         ARBITRAGE BOT - LIVE MONITORING DASHBOARD
═══════════════════════════════════════════════════════════════

📊 Last Updated: 2025-10-11 01:15:30
📂 Log File: logs/live_trading.log

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  CRITICAL HEALTH CHECK
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
✅ Custom(101) Errors (Code Bugs): 0 (PERFECT)

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  OPPORTUNITY PIPELINE (Recent Activity)
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
🔍 Opportunities Detected: 25
✅ Initial Simulations Passed: 20
⏳ Final Simulations Failed: 18 (market volatility)
🚀 JITO Bundles Submitted: 0 (TOTAL ALL TIME)

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  🎯 HOW CLOSE TO EXECUTION? (Your Key Metric)
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
📊 Execution Success Rate: 10.0%

🟠 STATUS: Finding opportunities, waiting for stability

📈 What this means:
   • 20 opportunities passed initial checks
   • 18 became stale before final execution
   • Need market window > 40-50ms for trade to land

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  💰 RECENT PROFITABLE OPPORTUNITIES
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
💵 Net Profit: 0.002788 SOL (21.9% retention)
💵 Net Profit: 0.003097 SOL (23.6% retention)
💵 Net Profit: 0.002788 SOL (21.9% retention)

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  🔥 LATEST OPPORTUNITY DETECTED
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
[INFO] 🔺 Triangle opportunity: ["SOL", "31fT1zWq", "SOL"] → 0.0139 SOL profit

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  📉 ERROR ANALYSIS
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
⚠️  Custom(3007) - Market Volatility: 15 (recent)
    └─ This is NORMAL - pools changing state

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
⌨️  Controls: Ctrl+C to exit | Refreshing every 2s
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

---

## 🚀 Quick Start

### Start Monitoring (Choose One)

**Full Dashboard:**
```bash
cd /home/tom14cat14/Arb_Bot/clean_arb_bot
./monitor_dashboard.sh
```

**Quick Status:**
```bash
cd /home/tom14cat14/Arb_Bot/clean_arb_bot
./quick_status.sh
```

**Watch Quick Status (Auto-refresh):**
```bash
watch -n 2 './quick_status.sh'
```

---

## 💡 Pro Tips

### 1. **Run Dashboard in Separate Terminal**

```bash
# Terminal 1: Bot running
tmux attach -t arb_bot

# Terminal 2: Monitoring dashboard
./monitor_dashboard.sh
```

**Zero impact on bot!** Dashboard reads logs only.

---

### 2. **Create Monitoring Cron Job**

```bash
# Check status every 5 minutes, log to file
*/5 * * * * cd /home/tom14cat14/Arb_Bot/clean_arb_bot && ./quick_status.sh >> monitoring_history.log 2>&1
```

**Creates history:** See how success rate changes over time

---

### 3. **Alert on Critical Errors**

```bash
#!/bin/bash
# Save as: alert_on_critical.sh

CRITICAL=$(grep -c "Custom(101)" logs/live_trading.log)

if [ "$CRITICAL" -gt 0 ]; then
    echo "ALERT: Critical errors detected! Stop bot immediately!"
    # Add your notification method here
    # Examples: send email, SMS, Discord webhook, etc.
fi
```

---

### 4. **Track Success Rate Over Time**

```bash
# Run periodically to see trends
while true; do
    echo "$(date '+%H:%M:%S') - $(./quick_status.sh)" >> success_rate_history.log
    sleep 300  # Every 5 minutes
done
```

**Analyze later:** See when market conditions are best

---

## 🎯 Understanding "How Close to Execution"

### The Execution Pipeline:

```
┌─────────────────────────────────────────────────────────┐
│ OPPORTUNITY DETECTED                                    │
│ └─ 0.003 SOL profit identified                         │
└─────────────────────────────────────────────────────────┘
                        ↓ T+0ms
┌─────────────────────────────────────────────────────────┐
│ BUILD SWAP INSTRUCTIONS                                 │
│ └─ 3 DEX swaps (SOL → Token → Token → SOL)            │
└─────────────────────────────────────────────────────────┘
                        ↓ T+15-30ms
┌─────────────────────────────────────────────────────────┐
│ INITIAL SIMULATION (Stage 1)                           │
│ └─ Check if instructions are valid                     │
│    ✅ PASSED (counted in "Simulations Passed")         │
└─────────────────────────────────────────────────────────┘
                        ↓ T+35ms
┌─────────────────────────────────────────────────────────┐
│ FINAL SIMULATION (Stage 2) - RIGHT BEFORE JITO         │
│ └─ Re-check pool state hasn't changed                  │
│    ❌ POOL CHANGED (counted in "Final Sims Failed")    │
│       OR                                                │
│    ✅ STILL VALID (JITO submission!)                   │
└─────────────────────────────────────────────────────────┘
                        ↓ T+40-50ms
┌─────────────────────────────────────────────────────────┐
│ 🚀 JITO BUNDLE SUBMITTED                               │
│ └─ Real trade on blockchain                            │
└─────────────────────────────────────────────────────────┘
```

**Success Rate** = How many make it from Stage 1 to JITO submission

**Higher % = Closer to execution**

---

## 🔍 What to Watch For

### Good Signs ✅

- Success Rate increasing over time
- Opportunities detected regularly (>5/min)
- Zero Custom(101) errors
- Recent profitable opportunities listed

### Concerning Signs ⚠️

- Success Rate stuck at 0% for hours
- No opportunities detected (<1/min)
- Custom(101) errors > 0 (CRITICAL)
- No recent profitable opportunities

### Action Items by Status:

**🟢 VERY CLOSE (>50% success rate):**
- Trade should land very soon
- Market stability good
- Keep monitoring

**🟡 GETTING CLOSE (20-50%):**
- Some stable windows appearing
- Be patient, trade will come
- Normal operation

**🟡 FINDING OPPS (1-20%):**
- Bot working correctly
- High market volatility
- May take longer for trade

**🔴 HIGH VOLATILITY (0%):**
- All opportunities going stale
- Extremely volatile market
- Bot protecting your capital correctly
- Wait for market to stabilize

---

## 📱 Remote Monitoring

### SSH + Quick Status

```bash
# From any device with SSH access
ssh user@your-server

cd /home/tom14cat14/Arb_Bot/clean_arb_bot
./quick_status.sh
```

**Perfect for:** Quick checks from phone, tablet, etc.

---

### Create Status API (Advanced)

```bash
# Add to cron: */1 * * * *
./quick_status.sh > /var/www/html/bot_status.txt

# Access from anywhere:
# https://your-server.com/bot_status.txt
```

**Security:** Add authentication if publicly accessible

---

## 🛠️ Troubleshooting

### Dashboard Not Updating

```bash
# Check if log file exists
ls -lh logs/live_trading.log

# Check if bot is writing to log
tail -f logs/live_trading.log

# Restart dashboard
Ctrl+C
./monitor_dashboard.sh
```

---

### Quick Status Shows "N/A"

**Means:** No recent data in logs

**Fix:**
```bash
# Verify bot is running
tmux list-sessions | grep arb_bot

# Check recent logs
tail -20 logs/live_trading.log

# Restart bot if needed
```

---

## 📊 Example Monitoring Session

**Good Session (Trade should land soon):**
```
01:00:00 - [🟡 FINDING OPPS] PassRate:15% | LastProfit:0.003 SOL
01:05:00 - [🟡 GETTING CLOSE] PassRate:35% | LastProfit:0.0028 SOL
01:10:00 - [🟢 VERY CLOSE] PassRate:55% | LastProfit:0.0031 SOL
01:15:00 - [🟢 TRADING] JITO:1 | LastProfit:0.0029 SOL ✅
```

**High Volatility Session (Normal, be patient):**
```
01:00:00 - [🔴 HIGH VOLATILITY] PassRate:0% | LastProfit:0.003 SOL
01:05:00 - [🔴 HIGH VOLATILITY] PassRate:0% | LastProfit:0.0028 SOL
01:10:00 - [🟡 FINDING OPPS] PassRate:5% | LastProfit:0.0031 SOL
01:15:00 - [🟡 GETTING CLOSE] PassRate:25% | LastProfit:0.0029 SOL
```

---

## 🎯 Summary

**Your Question:** How to watch profit margin without slowing bot?

**Answer:** Use the monitoring scripts!

1. **`./monitor_dashboard.sh`** - Full visual dashboard
2. **`./quick_status.sh`** - One-line status

**Both:**
- ✅ Read logs only (ZERO impact on bot)
- ✅ Show "how close to execution" (your key metric)
- ✅ Real-time updates
- ✅ Easy to use

**The bot runs at full speed, you watch from the side!**

---

**Created:** 2025-10-11
**Scripts:** `monitor_dashboard.sh`, `quick_status.sh`
**Performance Impact:** ZERO (log reading only)
**Recommended:** Keep dashboard open in separate terminal
