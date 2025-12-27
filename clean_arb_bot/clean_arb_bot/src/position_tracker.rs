// Position tracking system for capital management
//
// HIGH-4 FIX: Prevents over-leveraging by tracking capital in-flight
// Ensures concurrent opportunities don't exceed available capital
//
// Grok Cycle 3 Critical Fix: Atomic position tracking with lock-free design

use std::sync::atomic::{AtomicU64, Ordering};
use anyhow::{Result, anyhow};
use tracing::{info, warn, debug};

/// Lock-free position tracker using atomic operations
///
/// Thread-safe capital management for concurrent arbitrage opportunities
pub struct PositionTracker {
    /// Total capital available for trading (in lamports) - DYNAMICALLY UPDATED
    /// This tracks actual wallet balance minus fee reserve
    total_capital_lamports: AtomicU64,

    /// Capital currently committed to in-flight trades (atomic for thread-safety)
    in_flight_lamports: AtomicU64,

    /// Maximum allowed position size (in lamports)
    max_position_lamports: u64,

    /// Fee reserve (always protected, never tradeable) - DEFAULT: 0.1 SOL
    fee_reserve_lamports: u64,
}

impl PositionTracker {
    /// Create new position tracker with DYNAMIC balance tracking
    ///
    /// # Arguments
    /// * `capital_sol` - Initial trading capital in SOL (will update dynamically)
    /// * `max_position_sol` - Maximum position size per trade in SOL
    ///
    /// # Fee Reserve
    /// - 0.1 SOL is ALWAYS protected for transaction fees
    /// - Tradeable balance = wallet_balance - 0.1 SOL
    /// - This reserve is never used for trades
    pub fn new(capital_sol: f64, max_position_sol: f64) -> Self {
        const FEE_RESERVE_SOL: f64 = 0.1;
        let fee_reserve_lamports = (FEE_RESERVE_SOL * 1_000_000_000.0) as u64;

        // Initial capital (will be updated dynamically from wallet balance)
        let total_capital_lamports = (capital_sol * 1_000_000_000.0) as u64;
        let max_position_lamports = (max_position_sol * 1_000_000_000.0) as u64;

        info!("✅ Position tracker initialized (DYNAMIC SIZING):");
        info!("   Initial capital: {:.4} SOL ({} lamports)", capital_sol, total_capital_lamports);
        info!("   Max position: {:.4} SOL ({} lamports)", max_position_sol, max_position_lamports);
        info!("   Fee reserve: {:.4} SOL ({} lamports) - PROTECTED", FEE_RESERVE_SOL, fee_reserve_lamports);
        info!("   Tradeable balance will update based on actual wallet balance");

        Self {
            total_capital_lamports: AtomicU64::new(total_capital_lamports),
            in_flight_lamports: AtomicU64::new(0),
            max_position_lamports,
            fee_reserve_lamports,
        }
    }

    /// Check if we can open a new position of given size
    ///
    /// # Arguments
    /// * `size_lamports` - Desired position size in lamports
    ///
    /// # Returns
    /// true if capital is available, false otherwise
    pub fn can_open_position(&self, size_lamports: u64) -> bool {
        // Check against max position size limit
        if size_lamports > self.max_position_lamports {
            debug!("Position size {} exceeds max {} lamports",
                size_lamports, self.max_position_lamports);
            return false;
        }

        // Check against available capital (using atomic load)
        let current_in_flight = self.in_flight_lamports.load(Ordering::Relaxed);
        let total_capital = self.total_capital_lamports.load(Ordering::Relaxed);
        let available = total_capital.saturating_sub(current_in_flight);

        size_lamports <= available
    }

    /// Update total capital based on actual wallet balance
    ///
    /// # Arguments
    /// * `wallet_balance_lamports` - Current wallet balance from Solana RPC
    ///
    /// # Returns
    /// Tradeable balance (wallet balance - fee reserve)
    ///
    /// # Example
    /// ```
    /// // Wallet has 2.5 SOL
    /// let tradeable = tracker.update_from_wallet_balance(2_500_000_000);
    /// // tradeable = 2.5 - 0.1 = 2.4 SOL (2,400,000,000 lamports)
    /// ```
    pub fn update_from_wallet_balance(&self, wallet_balance_lamports: u64) -> u64 {
        // Calculate tradeable balance (wallet - fee reserve)
        let tradeable = wallet_balance_lamports.saturating_sub(self.fee_reserve_lamports);

        // Update total capital atomically
        let old_capital = self.total_capital_lamports.swap(tradeable, Ordering::Release);

        if tradeable != old_capital {
            let old_sol = old_capital as f64 / 1_000_000_000.0;
            let new_sol = tradeable as f64 / 1_000_000_000.0;
            let wallet_sol = wallet_balance_lamports as f64 / 1_000_000_000.0;

            info!("💰 Capital updated from wallet balance:");
            info!("   Wallet balance: {:.6} SOL", wallet_sol);
            info!("   Fee reserve: 0.1 SOL (protected)");
            info!("   Tradeable: {:.6} SOL (was {:.6} SOL)", new_sol, old_sol);
        }

        tradeable
    }

    /// Get dynamic position size based on current balance and opportunity size
    ///
    /// # Arguments
    /// * `opportunity_size_lamports` - Size of the arbitrage opportunity
    ///
    /// # Returns
    /// Actual position size to use: min(opportunity_size, tradeable_balance, max_position)
    ///
    /// # Logic
    /// - Use up to 100% of tradeable balance if opportunity is large enough
    /// - Cap at max_position_lamports for risk management
    /// - Cap at opportunity size (don't trade more than needed)
    pub fn get_dynamic_position_size(&self, opportunity_size_lamports: u64) -> u64 {
        let total_capital = self.total_capital_lamports.load(Ordering::Relaxed);
        let in_flight = self.in_flight_lamports.load(Ordering::Relaxed);
        let available = total_capital.saturating_sub(in_flight);

        // Use minimum of: opportunity size, available capital, max position
        let position_size = opportunity_size_lamports
            .min(available)
            .min(self.max_position_lamports);

        debug!("📊 Dynamic position sizing:");
        debug!("   Opportunity size: {:.6} SOL", opportunity_size_lamports as f64 / 1e9);
        debug!("   Available capital: {:.6} SOL", available as f64 / 1e9);
        debug!("   Max position: {:.6} SOL", self.max_position_lamports as f64 / 1e9);
        debug!("   Position size: {:.6} SOL", position_size as f64 / 1e9);

        position_size
    }

    /// Reserve capital for a new position (atomic operation)
    ///
    /// # Arguments
    /// * `amount_lamports` - Amount to reserve in lamports
    ///
    /// # Returns
    /// Ok(()) if reservation successful, Err if insufficient capital
    pub fn reserve_capital(&self, amount_lamports: u64) -> Result<()> {
        // Validate against max position size
        if amount_lamports > self.max_position_lamports {
            return Err(anyhow!(
                "Position size {} lamports exceeds max {} lamports ({:.4} SOL > {:.4} SOL)",
                amount_lamports,
                self.max_position_lamports,
                amount_lamports as f64 / 1_000_000_000.0,
                self.max_position_lamports as f64 / 1_000_000_000.0
            ));
        }

        // Atomic compare-and-swap loop
        // This ensures thread-safety without locks (lock-free programming)
        loop {
            let current = self.in_flight_lamports.load(Ordering::Acquire);
            let new_total = current + amount_lamports;
            let total_capital = self.total_capital_lamports.load(Ordering::Relaxed);

            // Check if we have enough capital
            if new_total > total_capital {
                let available = total_capital - current;
                return Err(anyhow!(
                    "Insufficient capital: {} lamports needed, {} lamports available ({:.4} SOL needed, {:.4} SOL available)",
                    amount_lamports,
                    available,
                    amount_lamports as f64 / 1_000_000_000.0,
                    available as f64 / 1_000_000_000.0
                ));
            }

            // Try to atomically update in_flight amount
            match self.in_flight_lamports.compare_exchange(
                current,
                new_total,
                Ordering::Release,  // Success: ensure write is visible to other threads
                Ordering::Relaxed,  // Failure: retry with new value
            ) {
                Ok(_) => {
                    debug!("✅ Reserved {} lamports ({:.4} SOL). In-flight: {} lamports ({:.4} SOL / {:.4} SOL total)",
                        amount_lamports,
                        amount_lamports as f64 / 1_000_000_000.0,
                        new_total,
                        new_total as f64 / 1_000_000_000.0,
                        self.total_capital_lamports.load(Ordering::Relaxed) as f64 / 1_000_000_000.0
                    );
                    return Ok(());
                }
                Err(_) => {
                    // Another thread modified in_flight_lamports, retry
                    debug!("⏳ Capital reservation conflict, retrying...");
                    continue;
                }
            }
        }
    }

    /// Release capital after position is closed
    ///
    /// # Arguments
    /// * `amount_lamports` - Amount to release in lamports
    ///
    /// SAFETY: This uses fetch_sub which can underflow if called incorrectly.
    /// Always ensure reserve_capital was called before release_capital.
    pub fn release_capital(&self, amount_lamports: u64) {
        let previous = self.in_flight_lamports.fetch_sub(amount_lamports, Ordering::Release);

        debug!("✅ Released {} lamports ({:.4} SOL). In-flight: {} lamports ({:.4} SOL / {:.4} SOL total)",
            amount_lamports,
            amount_lamports as f64 / 1_000_000_000.0,
            previous - amount_lamports,
            (previous - amount_lamports) as f64 / 1_000_000_000.0,
            self.total_capital_lamports.load(Ordering::Relaxed) as f64 / 1_000_000_000.0
        );

        // SAFETY CHECK: Detect potential underflow (should never happen if used correctly)
        if previous < amount_lamports {
            warn!("⚠️ CRITICAL: Position tracker underflow detected!");
            warn!("   Attempted to release {} lamports, but only {} were in-flight",
                amount_lamports, previous);
            warn!("   This indicates a logic bug - investigate immediately!");
        }
    }

    /// Get current capital utilization statistics
    pub fn get_stats(&self) -> PositionStats {
        let in_flight = self.in_flight_lamports.load(Ordering::Relaxed);
        let total_capital = self.total_capital_lamports.load(Ordering::Relaxed);
        let available = total_capital.saturating_sub(in_flight);
        let utilization_pct = (in_flight as f64 / total_capital as f64) * 100.0;

        PositionStats {
            total_capital_sol: total_capital as f64 / 1_000_000_000.0,
            in_flight_sol: in_flight as f64 / 1_000_000_000.0,
            available_sol: available as f64 / 1_000_000_000.0,
            utilization_pct,
            max_position_sol: self.max_position_lamports as f64 / 1_000_000_000.0,
        }
    }

    /// Emergency: Force reset all in-flight capital
    ///
    /// DANGER: Only use this for emergency recovery when positions are stuck
    /// This should NEVER be needed in normal operation
    pub fn emergency_reset(&self) {
        warn!("🚨 EMERGENCY: Force resetting all in-flight capital to zero");
        warn!("   Previous in-flight: {} lamports", self.in_flight_lamports.load(Ordering::Relaxed));

        self.in_flight_lamports.store(0, Ordering::Release);

        let total_capital = self.total_capital_lamports.load(Ordering::Relaxed);
        warn!("   All capital now available: {} lamports ({:.4} SOL)",
            total_capital,
            total_capital as f64 / 1_000_000_000.0
        );
    }
}

/// Position tracker statistics
#[derive(Debug, Clone)]
pub struct PositionStats {
    pub total_capital_sol: f64,
    pub in_flight_sol: f64,
    pub available_sol: f64,
    pub utilization_pct: f64,
    pub max_position_sol: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_can_open_position() {
        let tracker = PositionTracker::new(2.0, 0.5);

        // Can open position within limits
        assert!(tracker.can_open_position(500_000_000)); // 0.5 SOL
        assert!(tracker.can_open_position(100_000_000)); // 0.1 SOL

        // Cannot exceed max position
        assert!(!tracker.can_open_position(600_000_000)); // 0.6 SOL > 0.5 max

        // Cannot exceed total capital
        assert!(!tracker.can_open_position(3_000_000_000)); // 3 SOL > 2 total
    }

    #[test]
    fn test_reserve_and_release() {
        let tracker = PositionTracker::new(2.0, 0.5);

        // Reserve first position
        assert!(tracker.reserve_capital(500_000_000).is_ok()); // 0.5 SOL

        // Check available reduced
        assert!(tracker.can_open_position(500_000_000)); // Still have 1.5 SOL
        assert!(!tracker.can_open_position(2_000_000_000)); // But not 2 SOL

        // Reserve second position
        assert!(tracker.reserve_capital(500_000_000).is_ok()); // 0.5 SOL more

        // Now only 1 SOL left
        assert!(tracker.can_open_position(500_000_000));
        assert!(!tracker.can_open_position(1_500_000_000));

        // Release first position
        tracker.release_capital(500_000_000);

        // Should have 1.5 SOL available again
        assert!(tracker.can_open_position(1_000_000_000)); // 1 SOL ok
        assert!(!tracker.can_open_position(2_000_000_000)); // 2 SOL still too much
    }

    #[test]
    fn test_exceeds_capital() {
        let tracker = PositionTracker::new(1.0, 0.5);

        // Reserve 0.5 SOL
        assert!(tracker.reserve_capital(500_000_000).is_ok());

        // Reserve another 0.5 SOL
        assert!(tracker.reserve_capital(500_000_000).is_ok());

        // Try to reserve more - should fail (only 1 SOL total)
        let result = tracker.reserve_capital(100_000_000); // 0.1 SOL
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Insufficient capital"));
    }

    #[test]
    fn test_exceeds_max_position() {
        let tracker = PositionTracker::new(2.0, 0.5);

        // Try to reserve 0.6 SOL (exceeds max 0.5)
        let result = tracker.reserve_capital(600_000_000);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("exceeds max"));
    }

    #[test]
    fn test_stats() {
        let tracker = PositionTracker::new(2.0, 0.5);

        let stats = tracker.get_stats();
        assert_eq!(stats.total_capital_sol, 2.0);
        assert_eq!(stats.in_flight_sol, 0.0);
        assert_eq!(stats.available_sol, 2.0);
        assert_eq!(stats.utilization_pct, 0.0);

        // Reserve some capital
        tracker.reserve_capital(1_000_000_000).unwrap(); // 1 SOL

        let stats = tracker.get_stats();
        assert_eq!(stats.in_flight_sol, 1.0);
        assert_eq!(stats.available_sol, 1.0);
        assert_eq!(stats.utilization_pct, 50.0);
    }

    #[test]
    fn test_concurrent_reservations() {
        use std::sync::Arc;
        use std::thread;

        let tracker = Arc::new(PositionTracker::new(10.0, 1.0));
        let mut handles = vec![];

        // Spawn 20 threads, each trying to reserve 0.5 SOL
        for i in 0..20 {
            let tracker_clone = tracker.clone();
            let handle = thread::spawn(move || {
                tracker_clone.reserve_capital(500_000_000) // 0.5 SOL
            });
            handles.push(handle);
        }

        // Collect results
        let mut successes = 0;
        let mut failures = 0;
        for handle in handles {
            match handle.join().unwrap() {
                Ok(_) => successes += 1,
                Err(_) => failures += 1,
            }
        }

        // Total capital is 10 SOL, each reservation is 0.5 SOL
        // So exactly 20 should succeed (10 / 0.5 = 20)
        // Actually, exactly 10 SOL / 0.5 SOL = 20 reservations should succeed
        // Wait, 10 SOL total, 0.5 per reservation = max 20 reservations
        // But we started 20 threads, so all should succeed
        assert_eq!(successes, 20);
        assert_eq!(failures, 0);

        // Check final stats
        let stats = tracker.get_stats();
        assert_eq!(stats.in_flight_sol, 10.0); // All capital in use
        assert_eq!(stats.available_sol, 0.0);
        assert_eq!(stats.utilization_pct, 100.0);
    }
}
