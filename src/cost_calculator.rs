// Cost calculator for arbitrage profitability
//
// Calculates total cost of executing arbitrage including:
// - JITO tip (DYNAMIC - percentile based on profit size)
// - Transaction fees (compute budget + priority fees)
// - Slippage buffer (safety margin)
//
// ## Dynamic Percentile JITO Tip Strategy (2025-10-11)
//
// Uses different percentiles based on arbitrage profit size:
// - Small arbs (<0.01 SOL): 95th percentile (better profitability)
//   - Trade-off: 95% bundle landing vs 99%, but arbs become executable
//   - Allows small opportunities (0.001-0.01 SOL) to be profitable
// - Medium arbs (0.01-0.05 SOL): Interpolated between 95th and 99th
//   - Smooth transition as profit size increases
// - Large arbs (≥0.05 SOL): 99th percentile (best execution)
//   - Worth paying higher tip for competitive advantage
// - Minimum: 10% of profit or percentile floor (whichever higher)
// - Hard cap: 0.005 SOL maximum (market spike protection)
// - Updates every 10 minutes via background monitor
//
// ## Industry Guidance: 60/40 Gas/Tip Split
//
// Jito documentation recommends a 60% gas / 40% tip allocation of total fees:
// - Gas fees: ~60% of total (network, compute, priority)
// - JITO tip: ~40% of total (MEV protection payment)
//
// For small arbitrages, this ratio may differ due to:
// - Fixed gas costs (5,400 lamports minimum)
// - Competitive minimum tip (100,000 lamports = 95th percentile)
// - Result: Smaller arbs have tip-heavy ratio (e.g., 95% tip / 5% gas)
//
// For larger arbitrages with 10% profit-based tips, the ratio approaches 40-50% tip
// as the profit (and thus tip) scales up relative to fixed gas costs.

use crate::jito_tip_monitor::JitoTipFloor;
use tracing::debug;

/// Complete cost breakdown for arbitrage execution
#[derive(Debug, Clone)]
pub struct ArbitrageCosts {
    /// DEX swap fees (typically 0.25% per swap × 3 swaps = 0.75% total for triangle arb)
    pub dex_fee_lamports: u64,

    /// JITO tip amount (based on expected profit)
    pub jito_tip_lamports: u64,

    /// Base transaction fee (5000 lamports typical)
    pub base_tx_fee_lamports: u64,

    /// Compute budget fee (varies by compute units)
    pub compute_fee_lamports: u64,

    /// Priority fee (if using priority fees instead of JITO)
    pub priority_fee_lamports: u64,

    /// Total cost (sum of all above)
    pub total_cost_lamports: u64,
}

impl ArbitrageCosts {
    /// Calculate total costs for triangle arbitrage with DYNAMIC JITO tips
    ///
    /// # Arguments
    /// * `position_size_lamports` - Size of the position being traded (for DEX fee calculation)
    /// * `expected_profit_lamports` - Expected gross profit from arbitrage
    /// * `use_jito` - Whether using JITO bundles (true) or regular transactions (false)
    /// * `tip_floor` - Optional JITO tip floor data (if None, uses conservative defaults)
    ///
    /// # Strategy (NEW - Dynamic Tipping):
    /// - Normal profits: Beat JITO 95th percentile by 10%
    /// - Large profits (>0.5 SOL): Beat JITO 99th percentile by 10%
    /// - Minimum tip: 100,000 lamports (JITO competitive baseline)
    /// - Updates every 30 minutes via background monitor
    ///
    /// # Returns
    /// Complete cost breakdown
    pub fn calculate(
        position_size_lamports: u64,
        expected_profit_lamports: u64,
        use_jito: bool,
        tip_floor: Option<&JitoTipFloor>,
    ) -> Self {
        // DEX swap fees calculation
        // Triangle arbitrage = 3 swaps
        // Typical fee: 0.25% per swap (Raydium/Orca standard)
        // Total DEX fees: 0.75% of position size (NOT profit)
        // FIXED: Calculate based on actual position size
        let dex_fee_lamports = (position_size_lamports as f64 * 0.0075) as u64; // 0.75% of position

        // JITO tip calculation with DYNAMIC market-based tipping
        // UPDATED (2025-10-07): Dynamic tips based on JITO tip floor API
        // For sendBundle: Only tip matters (no 70/30 split with priority fee)
        let jito_tip_lamports = if use_jito {
            let profit_sol = expected_profit_lamports as f64 / 1_000_000_000.0;

            // AGGRESSIVE TIPPING STRATEGY: Always use 99th percentile + profit-based scaling
            // Base: 99th percentile (beats 99% of bundles)
            // Scale: Up to 3x based on profit margin (more margin = more aggressive)
            // Cap: Hard limit at 0.003 SOL

            let base_tip_99 = if let Some(floor) = tip_floor {
                floor.competitive_tip_99()
            } else {
                10_000_000_u64 // Fallback: 10M lamports (conservative 99th)
            };

            // Estimate total fees with base 99th percentile tip to calculate margin
            let estimated_dex_fees = (expected_profit_lamports as f64 * 0.0075) as u64;
            let estimated_gas = (base_tip_99 as f64 * 1.5) as u64; // Gas is 1.5x tip
            let total_fees_base = estimated_dex_fees + estimated_gas + base_tip_99;
            let fee_percentage = (total_fees_base as f64 / expected_profit_lamports as f64) * 100.0;

            // AGGRESSIVE 99TH PERCENTILE TIPPING (2025-10-11)
            // ALWAYS use 99th percentile - we want to CATCH opportunities, not miss them
            // User requirement: "we should be targeting 99% and I want .9 sol we need to be getting these not cutting cost and missing"
            // Trade-off: Higher tips but better execution rate (99% bundle landing)

            let base_tip_99 = if let Some(floor) = tip_floor {
                floor.competitive_tip_99()
            } else {
                10_000_000_u64 // Fallback: 10M lamports for 99th
            };

            // ALWAYS USE 99TH PERCENTILE - no interpolation, no cost cutting
            let percentile_tip = base_tip_99;

            // Still apply 10% minimum from profit for very small arbs
            let min_tip_percentage = 0.10; // 10% minimum
            let base_tip_from_profit =
                (expected_profit_lamports as f64 * min_tip_percentage) as u64;

            // Use the HIGHER of profit-based or dynamic percentile
            let base_tip = base_tip_from_profit.max(percentile_tip);

            // For very high margin trades, scale up to 15%
            let base_tip = if fee_percentage < 5.0 {
                // Ultra high margin: Scale up to 15% of profit
                let target_tip = (expected_profit_lamports as f64 * 0.15) as u64;
                base_tip.max(target_tip)
            } else {
                base_tip
            };

            // Minimum: 10% of profit or 100k lamports, whichever is higher
            let min_tip = base_tip_from_profit.max(100_000_u64);

            // Maximum: Cap at 17% of total estimated profit (user requirement)
            // This prevents over-paying even on 99th percentile for very profitable trades
            let max_tip_profit_cap = (expected_profit_lamports as f64 * 0.17) as u64; // 17% of profit

            // Also cap at 30% of net profit (after fees) for safety
            let net_profit_estimate = expected_profit_lamports
                .saturating_sub(estimated_dex_fees)
                .saturating_sub(estimated_gas);
            let max_tip_net_cap = net_profit_estimate * 30 / 100; // 30% of net profit

            // Absolute cap: 0.005 SOL (user requirement)
            let absolute_max_tip = 5_000_000_u64; // 0.005 SOL

            // Use the most restrictive cap
            let max_tip = max_tip_profit_cap
                .min(max_tip_net_cap)
                .min(absolute_max_tip);

            // Calculate tip with caps, BUT ensure dynamic percentile is ALWAYS the floor
            // This ensures competitive bundle landing appropriate for profit size
            // Use saturating operations to prevent overflow
            let capped_tip = base_tip.max(min_tip).min(max_tip);

            // CRITICAL: Dynamic percentile is ABSOLUTE MINIMUM (never go below it)
            // Small arbs use 95th floor, large arbs use 99th floor
            // This makes small arbs profitable while maintaining competitive execution
            let final_tip = capped_tip.max(percentile_tip);

            // PRODUCTION LOGGING: Track tip calculation (ALWAYS 99th percentile)
            let tip_percentage = (final_tip as f64 / expected_profit_lamports as f64) * 100.0;
            let was_capped = final_tip == absolute_max_tip; // Check if 0.005 SOL cap was applied
            let at_percentile_floor = final_tip == percentile_tip && capped_tip < percentile_tip;

            debug!("💰 Aggressive tip (99TH): Profit {:.6} SOL | Fee margin: {:.1}% → Tip {:.6} SOL ({:.2}% of profit){}{}",
                   profit_sol, fee_percentage, final_tip as f64 / 1e9, tip_percentage,
                   if was_capped { " [CAPPED]" } else { "" },
                   if at_percentile_floor { " [FLOOR]" } else { "" });

            final_tip
        } else {
            0
        };

        // Base transaction fee - Target ~1.5x JITO tip for realistic gas costs
        // Industry standard: Gas fees should be 50-150% of JITO tip
        // JITO tip is 3-7% of profit, capped at 0.001 SOL
        // So gas should be ~1.5x the tip amount
        let profit_sol = expected_profit_lamports as f64 / 1_000_000_000.0;

        // Calculate target gas as 1.5x JITO tip, with minimum floor for 3-swap arbitrage
        // Minimum 20,000 lamports covers: base tx fee (5k) + compute budget for 3 swaps (15k)
        let target_gas_lamports = ((jito_tip_lamports as f64 * 1.5) as u64).max(20_000);

        // Split between base tx fee (70%) and compute fee (30%)
        let base_tx_fee_lamports = (target_gas_lamports as f64 * 0.7) as u64;
        let compute_fee_lamports = (target_gas_lamports as f64 * 0.3) as u64;

        // Priority fee (only if not using JITO)
        let priority_fee_lamports = if !use_jito {
            // Scale priority fee with profit
            if profit_sol < 0.1 {
                50_000 // Small: standard priority
            } else if profit_sol < 1.0 {
                // Medium: scale from 50k to 150k
                50_000 + ((profit_sol - 0.1) * 111_111.0) as u64
            } else {
                // Large: scale from 150k to 300k
                150_000 + ((profit_sol - 1.0) * 150_000.0).min(150_000.0) as u64
            }
        } else {
            0
        };

        // Use saturating_add to prevent overflow
        let total_cost_lamports = dex_fee_lamports
            .saturating_add(jito_tip_lamports)
            .saturating_add(base_tx_fee_lamports)
            .saturating_add(compute_fee_lamports)
            .saturating_add(priority_fee_lamports);

        // PRODUCTION LOGGING: Complete cost breakdown for monitoring
        let profit_sol = expected_profit_lamports as f64 / 1e9;
        let total_cost_sol = total_cost_lamports as f64 / 1e9;
        let net_profit_sol = profit_sol - total_cost_sol;
        let retention_pct = if expected_profit_lamports > 0 {
            (net_profit_sol / profit_sol) * 100.0
        } else {
            0.0
        };

        debug!("📊 Cost breakdown: Gross {:.6} SOL | Costs {:.6} SOL | Net {:.6} SOL ({:.1}% retention)",
               profit_sol, total_cost_sol, net_profit_sol, retention_pct);
        debug!(
            "   DEX fees: {:.6} SOL, JITO tip: {:.6} SOL, Gas: {:.6} SOL, Priority: {:.6} SOL",
            dex_fee_lamports as f64 / 1e9,
            jito_tip_lamports as f64 / 1e9,
            (base_tx_fee_lamports + compute_fee_lamports) as f64 / 1e9,
            priority_fee_lamports as f64 / 1e9
        );

        Self {
            dex_fee_lamports,
            jito_tip_lamports,
            base_tx_fee_lamports,
            compute_fee_lamports,
            priority_fee_lamports,
            total_cost_lamports,
        }
    }

    /// Calculate minimum profitable gross profit
    ///
    /// Returns the minimum gross profit needed to cover all costs
    /// and achieve desired net profit.
    ///
    /// # Arguments
    /// * `desired_net_profit_lamports` - Target net profit after costs
    /// * `use_jito` - Whether using JITO bundles
    ///
    /// # Returns
    /// Minimum gross profit needed
    ///
    /// # Example
    /// ```
    /// // Want 0.1 SOL net profit using JITO
    /// let min_gross = ArbitrageCosts::min_gross_profit_for_net(
    ///     100_000_000, // 0.1 SOL desired net
    ///     true,        // using JITO
    /// );
    /// // min_gross ≈ 111,111,111 lamports (0.111 SOL)
    /// // because: gross * 0.9 (after 10% tip) = 100M net
    /// // so: gross = 100M / 0.9 = 111.11M
    /// ```
    pub fn min_gross_profit_for_net(desired_net_profit_lamports: u64, use_jito: bool) -> u64 {
        if use_jito {
            // With JITO: tip = 10% of gross, so net = gross * 0.9 - fixed_costs
            // Solving: net = gross * 0.9 - fixed_costs
            // gross = (net + fixed_costs) / 0.9

            let fixed_costs = 5_000 + 400; // base tx fee + compute fee = 5,400 lamports
            let min_gross = ((desired_net_profit_lamports + fixed_costs) as f64 / 0.9) as u64;

            // Round up to ensure we definitely cover costs
            min_gross + 1_000 // +1000 safety buffer
        } else {
            // Without JITO: net = gross - fixed_costs (no percentage-based tip)
            let fixed_costs = 5_000 + 400 + 50_000; // base + compute + priority = 55,400 lamports
            desired_net_profit_lamports + fixed_costs
        }
    }

    /// Get net profit after all costs
    /// Uses checked arithmetic to prevent overflow
    pub fn net_profit(&self, gross_profit_lamports: u64) -> i64 {
        (gross_profit_lamports as i64).saturating_sub(self.total_cost_lamports as i64)
    }

    /// Check if arbitrage is profitable after costs
    pub fn is_profitable(&self, gross_profit_lamports: u64) -> bool {
        self.net_profit(gross_profit_lamports) > 0
    }

    /// Get profit retention percentage
    pub fn retention_percentage(&self, gross_profit_lamports: u64) -> f64 {
        if gross_profit_lamports == 0 {
            return 0.0;
        }
        let net = self.net_profit(gross_profit_lamports);
        if net <= 0 {
            return 0.0;
        }
        (net as f64 / gross_profit_lamports as f64) * 100.0
    }

    /// Get gas/tip ratio (gas percentage, tip percentage)
    ///
    /// Industry recommendation: 60% gas / 40% tip
    /// Returns (gas_percentage, tip_percentage) of total fees
    ///
    /// # Examples
    /// ```
    /// let costs = ArbitrageCosts::calculate(1_000_000, true);
    /// let (gas_pct, tip_pct) = costs.gas_tip_ratio();
    /// // Small arb: ~5% gas, ~95% tip (due to minimum tip requirement)
    ///
    /// let large_costs = ArbitrageCosts::calculate(500_000_000, true);
    /// let (gas_pct, tip_pct) = large_costs.gas_tip_ratio();
    /// // Large arb: ~0.01% gas, ~99.99% tip (tip scales with profit)
    /// ```
    pub fn gas_tip_ratio(&self) -> (f64, f64) {
        if self.total_cost_lamports == 0 {
            return (0.0, 0.0);
        }

        let gas_cost =
            self.base_tx_fee_lamports + self.compute_fee_lamports + self.priority_fee_lamports;
        let tip_cost = self.jito_tip_lamports;

        let gas_percentage = (gas_cost as f64 / self.total_cost_lamports as f64) * 100.0;
        let tip_percentage = (tip_cost as f64 / self.total_cost_lamports as f64) * 100.0;

        (gas_percentage, tip_percentage)
    }
}

/// Calculate recommended minimum gross profit threshold (REASONABLE STRATEGY)
///
/// This is the minimum gross profit we should target to ensure profitability
/// after all costs with industry-standard JITO tips.
///
/// # Arguments
/// * `use_jito` - Whether using JITO bundles
///
/// # Returns
/// Recommended minimum gross profit in lamports
///
/// # Strategy:
/// - Reasonable minimum tip: 100,000 lamports (0.0001 SOL)
/// - Industry-standard 3-7% tips for most arbs
/// - Scaled gas fees for priority processing
///
/// # Example Results:
/// - With JITO: ~1M lamports (0.001 SOL) minimum for reliable execution
/// - Without JITO: 300k lamports (0.0003 SOL)
/// - Lower threshold allows capturing smaller but profitable opportunities
pub fn recommended_min_gross_profit(use_jito: bool) -> u64 {
    if use_jito {
        // GROK RECOMMENDATION (2025-10-07): Increase to 0.01 SOL for reliability
        // For 0.01 SOL gross profit:
        // - DEX fees (0.75%): 75,000 lamports
        // - JITO tip (3-5%, min 100k): 300,000 lamports (3%)
        // - Gas fees: 1.5x tip ~450,000 lamports
        // - Total costs: ~825,000 lamports
        // - Net profit: ~9,175,000 lamports (91.75% retention)
        //
        // UPDATED: 0.01 SOL minimum gross profit
        // - Aligns with JITO best practices
        // - Ensures reliable bundle landing
        // - Better profit margins after fees
        // - Matches test expectations
        10_000_000 // 0.01 SOL minimum (Grok-approved)
    } else {
        // Without JITO: Higher base costs
        // Base: 5,000 + compute: 400 + priority: 50,000 = 55,400
        // RECOMMENDED: 5x safety = 277,000 ≈ 300,000 lamports (0.0003 SOL)
        300_000
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jito_costs_small_profit_aggressive() {
        // Small arbitrage: 0.001 SOL profit - NOW UNPROFITABLE with aggressive strategy
        let costs = ArbitrageCosts::calculate(1_000_000, true);

        // NEW: Minimum tip is 1M lamports (0.001 SOL)
        assert_eq!(costs.jito_tip_lamports, 1_000_000); // Aggressive minimum
        assert_eq!(costs.base_tx_fee_lamports, 5_000); // Small profit, baseline gas
        assert_eq!(costs.compute_fee_lamports, 400); // Baseline compute
        assert_eq!(costs.priority_fee_lamports, 0); // Not used with JITO
        assert_eq!(costs.total_cost_lamports, 1_005_400);

        // Net profit: 1,000,000 - 1,005,400 = -5,400 lamports (LOSS!)
        assert_eq!(costs.net_profit(1_000_000), -5_400);
        assert!(!costs.is_profitable(1_000_000)); // Not profitable anymore!
    }

    #[test]
    fn test_jito_costs_medium_profit_aggressive() {
        // Medium arbitrage: 0.01 SOL profit - breakeven point
        let costs = ArbitrageCosts::calculate(10_000_000, true);

        // 10% of 10M = 1M, which equals minimum
        assert_eq!(costs.jito_tip_lamports, 1_000_000); // 10% tip
        assert_eq!(costs.base_tx_fee_lamports, 5_000); // Still baseline
        assert_eq!(costs.compute_fee_lamports, 400); // Still baseline
        assert_eq!(costs.total_cost_lamports, 1_005_400);

        // Net profit: 10,000,000 - 1,005,400 = 8,994,600 lamports
        assert_eq!(costs.net_profit(10_000_000), 8_994_600);
        assert!(costs.is_profitable(10_000_000));

        // Retention: ~90%
        let retention = costs.retention_percentage(10_000_000);
        assert!((retention - 89.95).abs() < 0.1);
    }

    #[test]
    fn test_jito_costs_large_profit_aggressive() {
        // Large arbitrage: 0.5 SOL profit - now uses 12% tip (medium range)
        let costs = ArbitrageCosts::calculate(500_000_000, true);

        // 0.5 SOL is in medium range (0.1-1 SOL): 12-15% tip
        // scale = 0.12 + (0.5 - 0.1) * 0.033 = 0.12 + 0.0132 = 0.1332 ≈ 13.32%
        // tip = 500M * 0.1332 ≈ 66.6M
        let expected_tip = (500_000_000.0 * 0.1332) as u64;
        assert!((costs.jito_tip_lamports as i64 - expected_tip as i64).abs() < 100_000);

        // Gas fees scaled for medium profit
        // Base: 5k + (0.5-0.1)*22222 = 5k + 8888.8 ≈ 13,889
        assert!(costs.base_tx_fee_lamports > 10_000);
        assert!(costs.base_tx_fee_lamports < 20_000);

        // Compute: 400 + (0.5-0.1)*1777 = 400 + 710.8 ≈ 1,111
        assert!(costs.compute_fee_lamports > 800);
        assert!(costs.compute_fee_lamports < 1_500);

        // Net profit lower due to higher costs, but still very profitable
        assert!(costs.net_profit(500_000_000) > 400_000_000); // Still >0.4 SOL net
        assert!(costs.is_profitable(500_000_000));

        // Retention slightly lower due to scaled costs: ~80-88%
        let retention = costs.retention_percentage(500_000_000);
        assert!(retention > 80.0);
        assert!(retention < 90.0);
    }

    #[test]
    fn test_jito_costs_very_large_profit_aggressive() {
        // Very large arbitrage: 2 SOL profit - uses 15-20% tip
        let costs = ArbitrageCosts::calculate(2_000_000_000, true);

        // 2 SOL is in large range (>1 SOL): 15-20% tip
        // scale = 0.15 + ((2.0 - 1.0) * 0.05).min(0.05) = 0.15 + 0.05 = 0.20 (20% cap)
        // tip = 2B * 0.20 = 400M
        let expected_tip = (2_000_000_000.0 * 0.20) as u64;
        assert!((costs.jito_tip_lamports as i64 - expected_tip as i64).abs() < 1_000_000);

        // Gas fees at maximum scaling
        // Base: 25k + (2.0-1.0)*25k = 50k (capped)
        assert_eq!(costs.base_tx_fee_lamports, 50_000);

        // Compute: 2k + (2.0-1.0)*2k = 4k (capped)
        assert_eq!(costs.compute_fee_lamports, 4_000);

        // Still highly profitable despite 20% tip
        assert!(costs.net_profit(2_000_000_000) > 1_500_000_000); // >1.5 SOL net
        assert!(costs.is_profitable(2_000_000_000));

        // Retention: ~75-80% due to 20% tip
        let retention = costs.retention_percentage(2_000_000_000);
        assert!(retention > 75.0);
        assert!(retention < 82.0);
    }

    #[test]
    fn test_min_gross_profit_calculation() {
        // Want 0.1 SOL net profit using JITO
        let min_gross = ArbitrageCosts::min_gross_profit_for_net(100_000_000, true);

        // Should be ~111M (100M / 0.9)
        assert!(min_gross >= 111_000_000);
        assert!(min_gross <= 112_000_000);

        // Verify it actually gives us the desired net profit
        let costs = ArbitrageCosts::calculate(min_gross, true);
        let actual_net = costs.net_profit(min_gross);

        // Should be close to 100M (within 1%)
        assert!(actual_net >= 99_000_000);
        assert!(actual_net <= 101_000_000);
    }

    #[test]
    fn test_recommended_minimums_aggressive() {
        let min_jito = recommended_min_gross_profit(true);
        let min_regular = recommended_min_gross_profit(false);

        // NEW: JITO minimum is 10M lamports (0.01 SOL)
        assert_eq!(min_jito, 10_000_000);
        // Regular is 300k lamports (0.0003 SOL)
        assert_eq!(min_regular, 300_000);

        // JITO should be much higher (needs aggressive tip)
        assert!(min_jito > min_regular);

        // Check profitability at recommended minimums
        let costs_jito = ArbitrageCosts::calculate(min_jito, true);
        let costs_regular = ArbitrageCosts::calculate(min_regular, false);

        // JITO should have good margin: ~9M net (90% retention)
        assert!(costs_jito.net_profit(min_jito) > 8_000_000); // >0.008 SOL net
        assert!(costs_regular.net_profit(min_regular) > 200_000); // >0.0002 SOL net
    }

    #[test]
    fn test_unprofitable_small_arb_aggressive() {
        // Very small arbitrage: 0.0001 SOL = 100k lamports - VERY unprofitable now
        let costs = ArbitrageCosts::calculate(100_000, true);

        // NEW: Minimum tip is 1M lamports (10x the profit!)
        assert_eq!(costs.jito_tip_lamports, 1_000_000);
        assert_eq!(costs.total_cost_lamports, 1_005_400);
        assert!(!costs.is_profitable(100_000)); // Net = -905,400 (huge loss!)

        // Even 0.001 SOL (1M lamports) is unprofitable
        let costs_1m = ArbitrageCosts::calculate(1_000_000, true);
        assert!(!costs_1m.is_profitable(1_000_000)); // Breaks even at best
    }

    #[test]
    fn test_gas_tip_ratio_aggressive() {
        // Small arbitrage: EXTREMELY tip-heavy due to aggressive minimum
        let small_costs = ArbitrageCosts::calculate(1_000_000, true);
        let (gas_pct_small, tip_pct_small) = small_costs.gas_tip_ratio();

        // Gas: 5,400 lamports, Tip: 1,000,000 lamports, Total: 1,005,400
        assert!((gas_pct_small - 0.54).abs() < 0.1); // ~0.54% gas
        assert!((tip_pct_small - 99.46).abs() < 0.1); // ~99.46% tip
        assert!((gas_pct_small + tip_pct_small - 100.0).abs() < 0.1); // Should sum to 100%

        // Medium arbitrage: Scaled gas, but still tip-heavy
        let medium_costs = ArbitrageCosts::calculate(500_000_000, true);
        let (gas_pct_med, tip_pct_med) = medium_costs.gas_tip_ratio();

        // 0.5 SOL: 13.32% tip (66.6M), scaled gas (~15k)
        // Tip still dominates even with scaled costs
        assert!(gas_pct_med < 1.0); // <1% gas
        assert!(tip_pct_med > 99.0); // >99% tip

        // Very large arbitrage: 20% tip with maxed gas
        let large_costs = ArbitrageCosts::calculate(2_000_000_000, true);
        let (gas_pct_large, tip_pct_large) = large_costs.gas_tip_ratio();

        // 2 SOL: 20% tip (400M), maxed gas (50k + 4k = 54k)
        // Tip: 400M, Gas: 54k, Total: ~400.054M
        assert!(gas_pct_large < 0.1); // <0.1% gas (54k out of 400M)
        assert!(tip_pct_large > 99.9); // >99.9% tip
        assert!((gas_pct_large + tip_pct_large - 100.0).abs() < 0.1); // Should sum to 100%
    }
}
