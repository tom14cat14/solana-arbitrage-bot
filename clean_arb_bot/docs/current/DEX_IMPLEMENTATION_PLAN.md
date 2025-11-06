# DEX Implementation Plan - Complete Coverage

**Date**: 2025-10-07
**Goal**: Implement all 18 DEXes from ShredStream service to unlock 100% arbitrage coverage

---

## 📊 Current Status

### ✅ Implemented (4 DEXes)
1. **Meteora DAMM V2** - `cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG` ✅
2. **Orca Whirlpools** - `whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc` ✅
3. **Raydium CPMM** - `CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C` ✅
4. **PumpSwap** - `GMk6j2defJhS7F194toqmJNFNhAkbDXhYJo5oR3Rpump` ✅

**Coverage**: 22% (4/18 DEXes)

### ❌ Missing (14 DEXes)

#### **Priority 1: Immediate Blockers (1 DEX)**
These are causing current execution failures:

1. **Meteora DAMM V1** - `Eo7WjKq67rjJQSZxS6z3YkapzY3eMj6Xy8X5EQVn5UaB` ❌
   - **Status**: BLOCKING CURRENT OPPORTUNITIES
   - **Error**: `Unknown DEX type: Meteora_DAMM_V1_5MDXXuJS`
   - **Impact**: HIGH - Bot detecting 0.02-0.03 SOL profit opportunities but can't execute

#### **Priority 2: High-Volume DEXes (5 DEXes)**
Major DEXes with significant trading volume:

2. **Raydium AMM V4** - `675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8` ❌
   - **Volume**: Top 3 DEX on Solana
   - **Impact**: HIGH - 30-40% more arbitrage opportunities

3. **Raydium CLMM** - `CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK` ❌
   - **Volume**: High (concentrated liquidity)
   - **Impact**: HIGH - Better capital efficiency

4. **Raydium Stable** - `5quBtoiQqxF9Jv6KYKctB59NT3gtJD2Y65kdnB1Uev3h` ❌
   - **Volume**: Medium (stablecoin swaps)
   - **Impact**: MEDIUM - Lower volatility, consistent arbs

5. **Meteora DLMM** - `LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo` ❌
   - **Volume**: High (dynamic liquidity)
   - **Impact**: HIGH - Advanced pool type

6. **Orca Legacy** - `9W959DqEETiGZocYWCQPaJ6sBmUzgfxXfqGeTEdp3aQP` ❌
   - **Volume**: Medium (older pools)
   - **Impact**: MEDIUM - Still active, complementary to Whirlpools

#### **Priority 3: Aggregator (1 DEX)**
7. **Jupiter** - `JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4` ❌
   - **Type**: Aggregator (routes across multiple DEXes)
   - **Impact**: MEDIUM - Already finds best prices, but useful for comparison

#### **Priority 4: Order Book DEX (1 DEX)**
8. **Serum** - `9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin` ❌
   - **Type**: Order book (not AMM)
   - **Impact**: MEDIUM - Different pricing model, unique opportunities

#### **Priority 5: Smaller DEXes (6 DEXes)**
Lower volume but may have unique arbitrage opportunities:

9. **Aldrin** - `AMM55ShdkoGRB5jVYPjWziwk8m5MpwyDgsMWHaMSQWH6` ❌
10. **Saros** - `SSwpkEEWHvCXCNWnMYXVW7gCYDXkF4aQMxKdpEqrZks` ❌
11. **Crema** - `6MLxLqiXaaSUpkgMnWDTuejNZEz3kE7k2woyHGVFw319` ❌
12. **Cropper** - `CTMAxxk34HjKWxQ3QLZQA1EQdxtjbYGP4Qjrw7nTn8bM` ❌
13. **Lifinity** - `EewxydAPCCVuNEyrVN68PuSYdQ7wKn27V9Gjeoi8dy3S` ❌
14. **Fluxbeam** - `FLUXBmPhT3Fd1EDVFdg46YREqHBeNypn1h4EbnTzWERX` ❌

**Impact**: LOW-MEDIUM - Less volume but may have price discrepancies

---

## 🎯 Implementation Strategy

### **Phase 1: Fix Immediate Blocker (30 minutes)**
**Target**: Meteora_DAMM_V1
- Add enum variant to DexType
- Create swap builder (similar to Raydium pattern)
- Test with current opportunity (0.02-0.03 SOL profit)

### **Phase 2: Major Raydium Variants (2-3 hours)**
**Target**: Raydium AMM V4, CLMM, Stable
- All use similar instruction format (discriminator `[143, 190, 90, 218...]`)
- Can reuse much of existing Raydium CPMM code
- May need different account structures

### **Phase 3: Meteora DLMM + Orca Legacy (1-2 hours)**
**Target**: Complete Meteora/Orca coverage
- Meteora DLMM: Advanced dynamic liquidity
- Orca Legacy: Similar to Whirlpools but older format

### **Phase 4: Jupiter + Serum (2-3 hours)**
**Target**: Special case DEXes
- Jupiter: Aggregator with multi-hop routes
- Serum: Order book, different instruction format

### **Phase 5: Remaining 6 DEXes (3-4 hours)**
**Target**: Complete coverage for all opportunities
- Aldrin, Saros, Crema, Cropper, Lifinity, Fluxbeam
- Lower priority but completes full coverage

**Total Estimated Time**: 8-12 hours for complete implementation

---

## 📋 Implementation Checklist Per DEX

For each DEX, we need:

### 1. **Type System Update**
- [ ] Add variant to `DexType` enum (`src/dex_swap/types.rs`)
- [ ] Add parsing logic in `from_dex_string()`
- [ ] Add program ID in `PoolRegistry::get_program_id()`

### 2. **Swap Builder Module**
- [ ] Create `src/dex_swap/{dex_name}.rs`
- [ ] Implement `{Dex}SwapBuilder` struct with:
  - `new()` - Constructor
  - `build_swap_instruction()` - Swap instruction building
  - `estimate_swap_output()` - Output estimation
- [ ] Add ghost pool protection (pre-flight checks)
- [ ] Implement pool state parsing
- [ ] Handle account derivation (PDAs, ATAs)

### 3. **Integration**
- [ ] Export module in `src/dex_swap/mod.rs`
- [ ] Add builder field to `SwapExecutor` struct
- [ ] Initialize builder in `SwapExecutor::new()`
- [ ] Wire up in `build_swap_instruction()` match
- [ ] Wire up in `estimate_swap_output()` match

### 4. **Testing**
- [ ] Compile successfully (0 errors)
- [ ] Test with real opportunities from ShredStream
- [ ] Verify swap instructions build correctly
- [ ] Validate output estimations accurate

---

## 🔧 Technical Details Per DEX

### **Meteora DAMM V1**
```
Program ID: Eo7WjKq67rjJQSZxS6z3YkapzY3eMj6Xy8X5EQVn5UaB
Discriminator: [248, 198, 158, 145, 225, 117, 135, 200] (same as Orca)
Pool Type: Dynamic AMM
Difficulty: LOW (can copy from Meteora V2 pattern)
```

### **Raydium AMM V4**
```
Program ID: 675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8
Discriminator: [143, 190, 90, 218, 196, 30, 51, 222]
Pool Type: Constant product (x*y=k)
Difficulty: LOW (similar to CPMM, already implemented)
```

### **Raydium CLMM**
```
Program ID: CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK
Discriminator: [143, 190, 90, 218, 196, 30, 51, 222]
Pool Type: Concentrated liquidity
Difficulty: MEDIUM (more complex pool state)
```

### **Raydium Stable**
```
Program ID: 5quBtoiQqxF9Jv6KYKctB59NT3gtJD2Y65kdnB1Uev3h
Discriminator: [143, 190, 90, 218, 196, 30, 51, 222]
Pool Type: Stable swap (low slippage for stablecoins)
Difficulty: MEDIUM (different fee curve)
```

### **Meteora DLMM**
```
Program ID: LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo
Discriminator: [248, 198, 158, 145, 225, 117, 135, 200]
Pool Type: Dynamic liquidity market maker
Difficulty: MEDIUM (advanced pool mechanics)
```

### **Orca Legacy**
```
Program ID: 9W959DqEETiGZocYWCQPaJ6sBmUzgfxXfqGeTEdp3aQP
Discriminator: [248, 198, 158, 145, 225, 117, 135, 200]
Pool Type: Simple AMM (older version)
Difficulty: LOW (simpler than Whirlpools)
```

### **Jupiter**
```
Program ID: JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4
Discriminator: [229, 23, 203, 151, 122, 227, 173, 42]
Pool Type: Aggregator (multi-hop routing)
Difficulty: HIGH (complex routing, multiple accounts)
```

### **Serum**
```
Program ID: 9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin
Discriminator: [0, 0, 0, 0, 0, 0, 0, 0] (generic)
Pool Type: Order book DEX
Difficulty: HIGH (order book vs AMM, different model)
```

### **Smaller DEXes (Aldrin, Saros, Crema, Cropper, Lifinity, Fluxbeam)**
```
Discriminator: [0, 0, 0, 0, 0, 0, 0, 0] (generic)
Difficulty: MEDIUM (need to research instruction formats)
Note: May require DEX-specific documentation/SDK
```

---

## 📈 Expected Impact

### **Current State (4 DEXes)**
- Opportunities: 1-2 per scan
- Coverage: ~22% of total DEX volume
- Execution failures: ~50% (missing DEX implementations)

### **After Phase 1 (5 DEXes - Add Meteora V1)**
- Opportunities: 3-5 per scan (+50%)
- Coverage: ~30% of total DEX volume
- Execution failures: ~30% (major blocker fixed)

### **After Phase 2 (8 DEXes - Add Raydium variants)**
- Opportunities: 8-12 per scan (+200%)
- Coverage: ~70% of total DEX volume
- Execution failures: ~15% (most major DEXes covered)

### **After Phase 3-5 (18 DEXes - Full coverage)**
- Opportunities: 15-25 per scan (+400%)
- Coverage: ~95% of total DEX volume
- Execution failures: <5% (complete coverage)

---

## 🚀 Implementation Order

**Immediate**:
1. Meteora_DAMM_V1 (Fix current blocker)

**Week 1**:
2. Raydium_AMM_V4
3. Raydium_CLMM
4. Raydium_Stable
5. Meteora_DLMM
6. Orca_Legacy

**Week 2**:
7. Jupiter
8. Serum

**Week 3**:
9-14. Remaining 6 smaller DEXes

---

## 🎯 Success Criteria

### **Phase 1 Complete**
- ✅ Meteora_DAMM_V1 opportunities execute successfully
- ✅ Current 0.02-0.03 SOL profit opportunities captured
- ✅ No more "Unknown DEX type" errors for Meteora V1

### **Phases 2-5 Complete**
- ✅ All 18 DEXes implemented
- ✅ 95%+ DEX volume coverage
- ✅ 15-25 opportunities per scan
- ✅ <5% execution failure rate
- ✅ Bot detecting opportunities across all major Solana DEXes

---

**Next Step**: Implement Meteora_DAMM_V1 to unblock current opportunities
