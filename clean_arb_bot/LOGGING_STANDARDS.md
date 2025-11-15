# Logging Standards - Arbitrage Bot

This document establishes consistent logging practices for the Solana Arbitrage Bot.

## Log Levels

### ERROR - Production Failures
**When to use**: Critical failures that require immediate attention

**Examples**:
- Failed to connect to critical services (RPC, JITO)
- Transaction failures after retries
- Unexpected panics or unrecoverable errors
- Circuit breaker trips
- Security violations

```rust
error!("❌ JITO bundle submission FAILED permanently");
error!("🚨 RPC CIRCUIT BREAKER TRIPPED: {} consecutive failures", failures);
```

### WARN - Recoverable Issues
**When to use**: Issues that are handled but noteworthy

**Examples**:
- Retry attempts
- Stale data detected
- Rate limiting encountered
- Simulation failures (expected)
- Configuration warnings

```rust
warn!("⚠️ ShredStream service error: {} - retrying in 1s", e);
warn!("⏰ Skipping stale opportunity (age: {}ms)", age.as_millis());
```

### INFO - Important Events
**When to use**: Key business logic events and state changes

**Examples**:
- Arbitrage opportunities detected
- Trades executed
- System startup/shutdown
- Configuration loaded
- Periodic statistics
- Capital updates

```rust
info!("🎯 Arbitrage opportunity found (age: {}ms)", age.as_millis());
info!("✅ JITO bundle submitted via gRPC (FAST!): {}", uuid);
info!("💰 Capital updated from wallet balance");
```

### DEBUG - Detailed Flow
**When to use**: Detailed execution flow for troubleshooting

**Examples**:
- Calculation details
- Filter rejections
- Queue operations
- Cost breakdowns
- Profitability checks

```rust
debug!("✅ PROFITABLE: {} - Spread {:.2}% >= {:.2}% required", token_mint, spread, min_spread);
debug!("📊 Dynamic position sizing: Opportunity {:.6} SOL", opportunity_size);
```

### TRACE - Verbose Details
**When to use**: Very detailed debugging (rarely used in production)

**Examples**:
- Transaction simulation logs
- Individual instruction details
- Raw API responses

```rust
trace!("RPC response: {:?}", response);
```

## Emoji Convention

Use emojis consistently to make logs scannable:

### Status Indicators
- ✅ Success / Completed
- ❌ Error / Failed
- ⚠️ Warning
- 🚨 Critical Alert
- ⏸️ Paused / Skipped
- 🧹 Cleanup

### Operations
- 🎯 Opportunity Detected
- 💰 Money / Capital / Profit
- 📡 Network / API Call
- 🚀 Launch / Submit
- 📤 Upload / Send
- 📥 Download / Receive
- 🔺 Triangle Arbitrage
- 💡 Insight / Discovery

### Metrics
- 📊 Statistics
- ⏱️ Timing
- 📈 Increase
- 📉 Decrease
- 🔒 Security / Lock

### System
- 🔧 Configuration
- 🔄 Retry
- ⏳ Waiting / In Progress
- 🔍 Search / Scan

## Message Format

### Structure
```
[EMOJI] [Brief description] ([context])
   [Detail 1]
   [Detail 2]
```

### Examples

**Good**:
```rust
info!("🎯 Arbitrage opportunity found (age: {}ms):", age);
info!("   Token: {}", token_mint);
info!("   Buy: {} @ {:.6} SOL", buy_dex, buy_price);
info!("   Sell: {} @ {:.6} SOL", sell_dex, sell_price);
info!("   Spread: {:.2}%", spread);
```

**Bad**:
```rust
info!("Found arbitrage for token {} buying at {} selling at {} spread {}",
    token_mint, buy_dex, sell_dex, spread);
```

### Number Formatting

- **SOL amounts**: `{:.6}` (6 decimals) for precision
- **Percentages**: `{:.2}%` (2 decimals) for readability
- **Lamports**: `{}` (no decimals) for raw values
- **Milliseconds**: `{}ms` (no decimals)
- **Counts**: `{}` (no decimals)

### Contextual Information

Always include relevant context:
- **Timing**: Age of data, elapsed time, timeouts
- **Amounts**: SOL values, lamports, counts
- **Identifiers**: Transaction signatures, token mints (truncated to 8 chars)
- **Status**: Success/failure, retry count, percentage

## Environment-Specific Logging

### Production
- Default level: `INFO`
- Minimal DEBUG logs
- Clear, actionable ERROR messages
- Statistics at regular intervals (60s)

### Development
- Default level: `DEBUG`
- More verbose profitability calculations
- Detailed filter reasoning
- Frequent statistics (10s)

### Testing
- Default level: `TRACE` for specific modules
- All simulation details
- Raw API responses

## Performance Considerations

### Avoid in Hot Paths
```rust
// BAD - String formatting happens even if debug is disabled
debug!("Heavy calculation: {}", expensive_function());

// GOOD - Only evaluate if debug enabled
if tracing::enabled!(tracing::Level::DEBUG) {
    debug!("Heavy calculation: {}", expensive_function());
}
```

### String Allocation
```rust
// BAD - Always allocates string
debug!("Token: {}", format!("{:.6}", value));

// GOOD - Format directly in macro
debug!("Token: {:.6}", value);
```

## Monitoring & Alerts

### Metrics to Extract from Logs

- **Error rate**: Count of ERROR level logs
- **Opportunity rate**: Count of "Arbitrage opportunity found"
- **Success rate**: Ratio of successful to failed submissions
- **Profitability**: Net profit from completed trades
- **Latency**: Age of opportunities when detected

### Alert Conditions

- ERROR rate > 10/minute → Investigate
- WARN "Circuit breaker" → Page on-call
- INFO "Trading halted" → Review configuration
- No opportunities for 5 minutes → Check data feed

## Examples by Module

### Arbitrage Engine
```rust
info!("🎯 Arbitrage opportunity found (age: {}ms):", age);
debug!("✅ PROFITABLE: {} - Spread {:.2}% >= {:.2}% required", token, spread, min);
warn!("⏰ Skipping stale opportunity (age: {}ms)", age);
```

### JITO Submitter
```rust
info!("🚀 JITO bundle submitted via gRPC: {}", uuid);
warn!("⚠️ 429 Rate Limit - Dropping trade (opportunity stale)");
error!("❌ JITO bundle submission FAILED permanently");
```

### RPC Client
```rust
info!("✅ Solana RPC client initialized: {}", url);
warn!("⚠️ Blockhash fetch attempt {} failed, retrying in {}ms", attempt, delay);
error!("🚨 RPC CIRCUIT BREAKER TRIPPED: {} consecutive failures", failures);
```

### Position Tracker
```rust
info!("💰 Capital updated from wallet balance:");
debug!("✅ Reserved {} lamports ({:.4} SOL)", amount, sol);
warn!("⚠️ CRITICAL: Position tracker underflow detected!");
```

## Review Checklist

Before committing, verify:
- [ ] Appropriate log level for message importance
- [ ] Emoji adds clarity and scannability
- [ ] Numbers formatted consistently
- [ ] Context included (timing, amounts, identifiers)
- [ ] No sensitive data (private keys, full addresses)
- [ ] No expensive operations in log arguments
- [ ] Multiline details properly indented
- [ ] Clear, actionable messages for errors

## References

- [tracing crate documentation](https://docs.rs/tracing)
- [Structured logging best practices](https://www.honeycomb.io/blog/structured-logging-and-your-team)
