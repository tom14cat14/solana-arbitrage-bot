# 📚 Arb Bot - Cross-DEX Arbitrage Documentation

**GitHub Repository**: https://github.com/tom14cat14/solana-arbitrage-bot

---

## ⚡ CORE RULES (Non-Negotiable)

### **1. Never Use Fake Data**
- ✅ Real blockchain data ONLY (ShredStream, Helius, RPC)
- ❌ NO simulated prices, NO random data, NO CoinGecko fallbacks
- ❌ NO `fastrand` for trading decisions
- **If data unavailable → Stop, don't fake it**

### **2. Fix Errors, Don't Shortcut Them**
- ✅ Root cause fixes ONLY
- ✅ Proper async/await, correct trait bounds, fix borrow checker
- ❌ NO hacks, NO `unsafe` blocks, NO suppressing warnings
- **If you don't understand the error → Research, don't guess**

### **3. Safety First, Always**
- ✅ Paper trading FIRST, every time
- ✅ All safety mechanisms working perfectly
- ✅ Complete fee accounting (gas + tips + DEX fees)
- ❌ NO "good enough" for money
- **Financial code must be bulletproof**

### **4. Real Money = Extra Caution**
- ✅ Test paper trading extensively before live
- ✅ Start with minimum positions (0.01-0.1 SOL)
- ✅ Monitor first 5-10 trades closely
- ✅ Circuit breakers must be tested
- **A single bug can cost significant money**

---

## 🎯 CURRENT STATUS

### **Clean Arb Bot - Production Ready** ✅
- **Status**: Live trading capable with real money
- **Location**: `/clean_arb_bot/`
- **GitHub**: https://github.com/tom14cat14/solana-arbitrage-bot

### **Recent Cleanup (2025-11-06)**
✅ **Major codebase reorganization completed:**
- Removed 140+ unnecessary files
- Flattened `src/dex_swap/` modules into `src/`
- Organized documentation (12 essential docs in `docs/current/`)
- Moved scripts to `scripts/` folder
- 90% cleaner directory structure
- All code compiles successfully

---

## 📖 Documentation

All essential documentation is in `/clean_arb_bot/docs/current/`:

1. **CURRENT_STATUS_2025_10_07.md** - Latest status and configuration
2. **AUTONOMOUS_TRADING_ACTIVE.md** - Bot control and monitoring
3. **GRPC_SUCCESS.md** - gRPC implementation (2x faster JITO)
4. **JITO_GRPC_FINDINGS.md** - JITO bundle submission details
5. **REALISTIC_FILTERS_COMPLETE.md** - 97%+ junk elimination filters
6. **VOLUME_FILTER_FINDINGS.md** - Volume-based filtering strategy
7. **HUMIDIFI_ENABLED.md** - HumidiFi DEX integration
8. **PUMPSWAP_FIXED_2025_10_14.md** - PumpSwap implementation
9. **DEX_IMPLEMENTATION_PLAN.md** - DEX coverage roadmap
10. **DEPLOYMENT_PLAN.md** - Production deployment guide
11. **MONITORING_GUIDE.md** - Monitoring and observability
12. **LIVE_TRADING_GUIDE.md** - Live trading setup

---

## 🚀 Quick Start

```bash
cd clean_arb_bot

# Build
~/.cargo/bin/cargo build --release

# Paper Trading (safe)
env ENABLE_REAL_TRADING=false PAPER_TRADING=true \
  ./target/release/clean_arb_bot

# Live Trading (caution!)
env ENABLE_REAL_TRADING=true PAPER_TRADING=false \
  ./target/release/clean_arb_bot
```

---

## 📁 Repository Structure

```
solana-arbitrage-bot/
├── clean_arb_bot/           # Main production bot
│   ├── src/                 # Rust source (flattened modules)
│   ├── docs/current/        # Essential documentation
│   ├── scripts/             # Shell scripts
│   ├── production/          # Monitoring scripts
│   └── Cargo.toml
├── CLAUDE.md               # This file
├── README.md               # GitHub README
└── .gitignore
```

---

## ⚠️ IMPORTANT

- **Real money trading** requires extensive paper trading validation first
- **Circuit breakers** must be tested before live deployment
- **JITO rate limits** (1 bundle/~1s) are shared across all bots
- See documentation in `clean_arb_bot/docs/current/` for details

---

**Last Updated**: 2025-11-06
**Status**: Clean, organized, production-ready codebase
