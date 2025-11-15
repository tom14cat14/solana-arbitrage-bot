# TODO Items - Technical Debt Tracking

This file tracks known TODO items in the codebase. Each item includes location, description, and priority.

## High Priority

### 1. Jupiter Swap Transaction Building
**Location**: `src/arbitrage_engine.rs:498`
**Description**: Build actual Jupiter swap transaction for triangle arbitrage
**Status**: Placeholder - Jupiter integration not yet complete
**Impact**: Triangle arbitrage via Jupiter is not functional
**Effort**: Medium - requires Jupiter API integration for transaction building

### 2. Refactor JITO Bundle Tip Inclusion
**Location**: `src/jito_bundle_client.rs:317, 462`
**Description**: Include JITO tip INSIDE swap transaction instead of separate instruction
**Status**: Current approach uses deprecated legacy method
**Impact**: Better MEV protection by preventing unbundling
**Effort**: High - requires refactoring swap_executor to build instructions without signing
**Note**: Current implementation works but could be more secure

### 3. Jupiter Swap Instruction Token Mints
**Location**: `src/swap_executor.rs:558`
**Description**: Fetch actual token mints from pool account data instead of hardcoding
**Status**: Currently uses default/hardcoded values
**Impact**: May fail for non-standard pools
**Effort**: Low - parse pool data structure

## Medium Priority

### 4. Meteora Function Signature Update
**Location**: `src/meteora.rs:292`
**Description**: Update function signature to return Vec<Instruction>
**Status**: Current signature works but could be cleaner
**Impact**: API consistency
**Effort**: Low - refactor return type

### 5. Orca Function Signature Update
**Location**: `src/orca.rs:272`
**Description**: Update function signature to return Vec<Instruction>
**Status**: Current signature works but could be cleaner
**Impact**: API consistency
**Effort**: Low - refactor return type

### 6. Raydium Function Signature Update
**Location**: `src/raydium.rs:287`
**Description**: Update function signature to return Vec<Instruction>
**Status**: Current signature works but could be cleaner
**Impact**: API consistency
**Effort**: Low - refactor return type

## Low Priority

### 7. Pool Registry API Integration
**Location**: `src/pool_registry.rs:221, 236`
**Description**: Get pool token mints from API instead of defaults
**Status**: Currently uses default placeholders
**Impact**: Limited - hardcoded pools work fine
**Effort**: Low - API call + parsing

### 8. Implement getProgramAccounts with Prefix Filter
**Location**: `src/pool_registry.rs:391`
**Description**: Use RPC getProgramAccounts for dynamic pool discovery
**Status**: Not implemented - uses hardcoded pools
**Impact**: Limited pool coverage
**Effort**: Medium - RPC call + data parsing + filtering

### 9. Add Known Meteora Pool Addresses
**Location**: `src/pool_registry.rs:424`
**Description**: Expand hardcoded Meteora pool list
**Status**: Basic pools covered
**Impact**: More arbitrage opportunities
**Effort**: Low - just add more addresses

### 10. Add Known Orca Pool Addresses
**Location**: `src/pool_registry.rs:449`
**Description**: Expand hardcoded Orca pool list
**Status**: Basic pools covered
**Impact**: More arbitrage opportunities
**Effort**: Low - just add more addresses

### 11. Batch Account Fetching
**Location**: `src/pool_registry.rs:518`
**Description**: Use get_multiple_accounts for batch fetching instead of individual calls
**Status**: Works but inefficient
**Impact**: Performance improvement
**Effort**: Low - use existing RPC method

## Completed / Closed

_None yet_

## Implementation Priority Order

Based on impact and effort:

1. **High Priority, Medium Effort**: Jupiter swap transaction building (#1)
2. **High Priority, High Effort**: JITO tip refactoring (#2) - do when time permits
3. **Low Priority, Low Effort**: Batch improvements (#3, #7, #9, #10, #11)
4. **Medium Priority, Medium Effort**: Dynamic pool discovery (#8)
5. **Medium Priority, Low Effort**: Function signature updates (#4, #5, #6)

## Notes

- Most TODOs are non-critical and the bot functions well without them
- Focus on high-impact items first
- Several items are optimizations rather than bug fixes
- Jupiter integration (#1) would unlock additional arbitrage paths
