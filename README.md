# Solana Cross-DEX Arbitrage Bot

Production-ready arbitrage bot for Solana, executing triangular arbitrage opportunities across multiple DEXs.

## 📊 Current Status

**Production Ready** - Clean, optimized codebase with 90% fewer files after major cleanup (Nov 2025)

## 🚀 Quick Start

```bash
cd clean_arb_bot

# Build
~/.cargo/bin/cargo build --release

# Paper Trading (safe testing)
env ENABLE_REAL_TRADING=false PAPER_TRADING=true \
  ./target/release/clean_arb_bot

# Live Trading (requires funded wallet)
env ENABLE_REAL_TRADING=true PAPER_TRADING=false \
  ./target/release/clean_arb_bot
```

## 📁 Repository Structure

```
solana-arbitrage-bot/
├── clean_arb_bot/           # Production bot
│   ├── src/                 # Rust source code
│   ├── docs/current/        # Essential documentation
│   ├── scripts/             # Utility scripts
│   ├── production/          # Deployment scripts
│   └── Cargo.toml
├── CLAUDE.md               # Complete documentation
└── README.md               # This file
```

## 📖 Documentation

All essential documentation is in `clean_arb_bot/docs/current/`:

- **CURRENT_STATUS_2025_10_07.md** - Latest status and configuration
- **AUTONOMOUS_TRADING_ACTIVE.md** - Bot control and monitoring
- **GRPC_SUCCESS.md** - gRPC implementation (2x faster)
- **JITO_GRPC_FINDINGS.md** - JITO bundle submission
- **LIVE_TRADING_GUIDE.md** - Production deployment

See [CLAUDE.md](CLAUDE.md) for complete documentation.

## ⚡ Key Features

- **Real-time opportunity detection** via ShredStream (<15ms latency)
- **Multi-DEX support**: Meteora, Orca, Raydium, PumpSwap, HumidiFi
- **JITO bundles** with dynamic tipping (99.3% cost reduction)
- **Advanced filtering** - 97%+ junk elimination
- **Safety first** - Comprehensive circuit breakers and monitoring

## ⚠️ Important Notes

- **Real money trading** requires extensive paper trading validation first
- **JITO rate limits** (1 bundle/~1s) are shared across all bots
- See documentation for complete safety guidelines

## 📜 License

Private repository - All rights reserved

---

**Last Updated**: 2025-11-06
**Status**: Production ready after major cleanup
