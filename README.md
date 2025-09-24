# Arbitrage Bot Directory

## 💰 Arbitrage Bot Overview

This directory contains the **Cross-DEX Arbitrage Bot** - a specialized trading bot focused exclusively on arbitrage opportunities across different Solana DEXs.

## 🎯 Arbitrage Bot Strategy

### **Core Focus**: Cross-DEX Price Differences
**Location**: `/home/ubuntu/projects/arb-bots/arb-bot/src/main.rs`

**Strategy**: Patient, consistent arbitrage trading across multiple DEXs
- ✅ **Arbitrage**: PRIMARY and ONLY strategy
- ❌ **Sandwich attacks**: DISABLED (handled by MEV bot)
- ❌ **Liquidations**: DISABLED (handled by MEV bot)
- ⏱️ **Timeout**: 3000ms (more patient than MEV bots)
- 🔄 **Concurrency**: 15 opportunities (higher than MEV - lower risk)
- 📈 **Risk Profile**: Lower risk, consistent profits

### **Key Characteristics**
- **Specialization**: Pure arbitrage focus without MEV competition
- **Patience**: Longer execution timeouts for better fill rates
- **Volume**: Higher concurrent opportunities for steady profits
- **Safety**: Lower risk profile compared to MEV strategies
- **Consistency**: Designed for reliable, repeatable gains

## 🏗️ Arbitrage Bot Architecture

### **Main Components**

#### **Primary Bot File**
- **Location**: `/home/ubuntu/projects/arb-bots/arb-bot/src/main.rs`
- **Function**: Main arbitrage bot execution
- **Strategy**: Cross-DEX price difference detection and execution

#### **Shared Infrastructure Dependencies**
**Base Location**: `/home/ubuntu/projects/shared/shredstream-shared/src/`

**Core Arbitrage Components**:
- **Arbitrage Engine**: `arbitrage_engine.rs` - Core arbitrage logic and opportunity detection
- **DEX Registry**: `dex_registry.rs` - Multi-DEX support and price comparison
- **Route Cache**: `route_cache.rs` - Optimized routing between DEXs
- **Jupiter Executor**: `jupiter_executor.rs` - Cross-DEX swap execution
- **Mempool Monitor**: `mempool_monitor.rs` - Real-time opportunity detection

**Supporting Infrastructure**:
- **Bot Coordinator**: `bot_coordinator.rs` - Coordinates with MEV bots to avoid conflicts
- **Database Tracker**: `database_tracker.rs` - Performance and profit tracking
- **Jupiter Rate Limiter**: `jupiter_rate_limiter.rs` - API rate limiting
- **Transaction Processor**: `transaction_processor.rs` - Transaction management
- **Wallet Manager**: `wallet_manager.rs` - Multi-wallet balance management
- **Dynamic Fee Model**: `dynamic_fee_model.rs` - Optimal fee calculation

## 🔧 Arbitrage Bot Configuration

### **Arbitrage-Specific Settings**
```rust
let arb_config = MonitorConfig {
    enable_sandwich_attacks: false,  // DISABLED: Handled by MEV bot
    enable_arbitrage: true,          // PRIMARY: Cross-DEX arbitrage
    enable_liquidations: false,      // DISABLED: Handled by MEV bot
    max_concurrent_opportunities: 15, // Higher concurrency (lower risk)
    opportunity_timeout_ms: 3000,    // Longer timeout (patience pays)
    stats_reporting_interval_ms: 30000, // 30 second reports
};
```

### **Strategy Philosophy**
- **Separation of Concerns**: Focused only on arbitrage, no MEV interference
- **Risk Management**: Lower risk through longer timeouts and patience
- **Volume Strategy**: More concurrent trades at lower individual risk
- **Consistency**: Reliable profits through proven arbitrage methods

## 📊 Supported DEXs for Arbitrage

The arbitrage bot monitors price differences across:
- **Raydium**: High liquidity AMM
- **Orca**: Concentrated liquidity pools
- **Jupiter**: Aggregated routing
- **Whirlpool**: Concentrated liquidity (Orca's new model)
- **Meteora**: Dynamic liquidity pools
- **Saber**: Stable coin focused
- **Aldrin**: Additional liquidity source
- **Serum**: Order book DEX

## 🚀 How to Run the Arbitrage Bot

### **Quick Start**
```bash
# Navigate to arbitrage bot directory
cd /home/ubuntu/projects/arb-bots/arb-bot

# Run the arbitrage bot
cargo run
```

### **Environment Setup**
The arbitrage bot uses the same environment configuration as MEV bots:
**Config File**: `/home/ubuntu/projects/shared/shredstream-shared/.env`

**Key Settings**:
- **ShredStream**: Real-time price feeds
- **Jupiter API**: Cross-DEX routing
- **Paper Trading**: Enabled by default for safety
- **RPC Endpoint**: Solana mainnet connection

## 📈 Arbitrage Performance Metrics

### **Real-time Statistics**
The arbitrage bot provides detailed statistics:
- **Transactions Processed**: Total arbitrage attempts
- **Opportunities Detected**: Price differences found
- **Opportunities Executed**: Successful arbitrage trades
- **Total Profit**: Cumulative profit in SOL
- **Success Rate**: Execution success percentage
- **Average Processing Time**: Speed metrics

### **24-Hour Reports**
Comprehensive performance analysis including:
- **Total Opportunities**: Arbitrage chances detected
- **Total Executions**: Successful trades completed
- **Profit Breakdown**: Profit per DEX pair
- **Success Rate Analysis**: Performance by DEX and token pair
- **Average Execution Time**: Speed optimization metrics

## 🔄 Bot Coordination

### **Multi-Bot Ecosystem**
The arbitrage bot works in coordination with:
- **MEV Bot**: Handles sandwich attacks and liquidations
- **Micro-Cap MEV Bot**: Handles small market cap opportunities
- **Bot Coordinator**: Manages resource allocation and prevents conflicts

### **Resource Management**
- **Jupiter API**: Shared rate limiting across all bots
- **Wallet Management**: Coordinated balance tracking
- **Opportunity Prioritization**: High-value MEV gets priority
- **Route Optimization**: Shared route cache for efficiency

## 🎯 Arbitrage Strategy Details

### **Opportunity Detection**
1. **Price Monitoring**: Real-time price feeds from all supported DEXs
2. **Spread Analysis**: Identifies profitable price differences
3. **Liquidity Check**: Ensures sufficient liquidity for execution
4. **Fee Calculation**: Accounts for all trading fees and slippage
5. **Profit Validation**: Confirms net positive outcome

### **Execution Process**
1. **Route Planning**: Optimal path from DEX A to DEX B
2. **Transaction Building**: Atomic swap transactions
3. **Fee Optimization**: Dynamic fee calculation for speed
4. **Execution**: Simultaneous buy/sell across DEXs
5. **Monitoring**: Track execution and calculate actual profit

## 📝 Technical Notes

### **Arbitrage vs MEV**
- **Arbitrage**: Price differences between DEXs (lower risk)
- **MEV**: Sandwich attacks and liquidations (higher risk, higher reward)
- **Separation**: Prevents strategy interference and optimizes each approach

### **Performance Optimization**
- **Route Caching**: Pre-computed optimal routes
- **Rate Limiting**: Efficient Jupiter API usage
- **Concurrent Execution**: Multiple opportunities simultaneously
- **Fee Modeling**: Dynamic fee calculation for profitability

### **Safety Features**
- **Paper Trading**: Default mode for testing
- **Slippage Protection**: Maximum slippage limits
- **Balance Monitoring**: Prevents over-trading
- **Error Recovery**: Robust error handling and retry logic

---

**💰 Specialized arbitrage bot designed for consistent, low-risk profits through cross-DEX trading!**