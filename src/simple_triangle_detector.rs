use std::collections::HashMap;
use tracing::{debug, info};

use crate::shredstream_client::TokenPrice;

/// Simple triangle opportunity detected from ShredStream data
#[derive(Debug, Clone)]
pub struct SimpleTriangleOpportunity {
    pub token_a_mint: String,
    pub token_b_mint: String,
    pub dex_1: String, // SOL → TokenA
    pub dex_2: String, // TokenA → TokenB
    pub dex_3: String, // TokenB → SOL

    // GHOST POOL FIX: Full 44-char pool addresses from ShredStream
    pub pool_1_address: String, // Full address for SOL → TokenA pool
    pub pool_3_address: String, // Full address for TokenB → SOL pool

    pub profit_sol: f64,
    pub profit_percentage: f64,
    pub input_amount_sol: f64,
}

/// Simple triangle detector using only ShredStream price data
/// Detects: SOL → TokenA → TokenB → SOL
pub struct SimpleTriangleDetector {
    sol_mint: String,
}

impl SimpleTriangleDetector {
    pub fn new() -> Self {
        Self {
            sol_mint: "So11111111111111111111111111111111111111112".to_string(),
        }
    }

    /// Find triangle opportunities from ShredStream prices
    /// Strategy: Find pairs where SOL → A → B → SOL is profitable
    pub fn find_opportunities(
        &self,
        prices: &HashMap<String, TokenPrice>,
        capital_sol: f64,
        config: &crate::config::Config,
    ) -> Vec<SimpleTriangleOpportunity> {
        let mut opportunities = Vec::new();

        // Group prices by token mint
        let mut token_prices: HashMap<String, Vec<&TokenPrice>> = HashMap::new();
        for price in prices.values() {
            token_prices
                .entry(price.token_mint.clone())
                .or_insert_with(Vec::new)
                .push(price);
        }

        // Filter out spam/dead tokens with too many pools
        // Real tokens typically have 1-10 pools across DEXes
        // Tokens with 50+ pools are likely spam, dead PumpFun tokens, or data errors
        const MAX_POOLS_PER_TOKEN: usize = 50;
        token_prices.retain(|token_mint, prices_list| {
            let pool_count = prices_list.len();
            if pool_count > MAX_POOLS_PER_TOKEN {
                debug!(
                    "🚫 Filtering out spam token {} with {} pools (max: {})",
                    &token_mint[0..8.min(token_mint.len())],
                    pool_count,
                    MAX_POOLS_PER_TOKEN
                );
                false
            } else {
                true
            }
        });

        // Get all tokens with SOL pairs
        let tokens_with_sol_pairs: Vec<&String> = token_prices.keys().collect();

        debug!(
            "🔍 Scanning {} tokens for triangle paths",
            tokens_with_sol_pairs.len()
        );

        // Try all combinations: SOL → TokenA → TokenB → SOL
        for (i, token_a_mint) in tokens_with_sol_pairs.iter().enumerate() {
            if *token_a_mint == &self.sol_mint {
                continue;
            }

            // Check first 500 tokens (increased from 100 to find more opportunities)
            if i >= 500 {
                break;
            }

            let token_a_prices = &token_prices[*token_a_mint];

            for token_b_mint in &tokens_with_sol_pairs {
                if token_b_mint == token_a_mint || *token_b_mint == &self.sol_mint {
                    continue;
                }

                let token_b_prices = &token_prices[*token_b_mint];

                // Try to find a profitable path
                if let Some(opp) = self.calculate_triangle_profit(
                    token_a_mint,
                    token_b_mint,
                    token_a_prices,
                    token_b_prices,
                    capital_sol,
                    config,
                ) {
                    opportunities.push(opp);

                    // Limit to 50 opportunities (increased to see more)
                    if opportunities.len() >= 50 {
                        return opportunities;
                    }
                }
            }
        }

        if !opportunities.is_empty() {
            info!("🎯 Found {} triangle opportunities", opportunities.len());
        }

        opportunities
    }

    /// Calculate profit for SOL → TokenA → TokenB → SOL
    fn calculate_triangle_profit(
        &self,
        token_a_mint: &str,
        token_b_mint: &str,
        token_a_prices: &[&TokenPrice],
        token_b_prices: &[&TokenPrice],
        capital_sol: f64,
        config: &crate::config::Config,
    ) -> Option<SimpleTriangleOpportunity> {
        // Try all combinations of DEXs
        for price_a in token_a_prices {
            for price_b in token_b_prices {
                // Step 1: SOL → TokenA
                let fee_1 = 0.003; // 0.3% typical DEX fee
                let token_a_amount = (capital_sol * (1.0 - fee_1)) / price_a.price_sol;

                // Step 2: TokenA → TokenB
                // We need to know the TokenA/TokenB price
                // Approximate: value_in_sol / price_b
                let token_a_value_sol = token_a_amount * price_a.price_sol;
                let fee_2 = 0.003;
                let token_b_amount = (token_a_value_sol * (1.0 - fee_2)) / price_b.price_sol;

                // Step 3: TokenB → SOL
                let fee_3 = 0.003;
                let sol_received = token_b_amount * price_b.price_sol * (1.0 - fee_3);

                // Gross profit after DEX fees (0.9% total)
                let gross_profit = sol_received - capital_sol;
                let profit_pct = (gross_profit / capital_sol) * 100.0;

                // Calculate total fees (JITO tip + gas + compute)
                let total_fees = config.calculate_total_fees(gross_profit);

                // Calculate net profit after ALL fees
                let net_profit = gross_profit - total_fees;

                // Calculate required margin (UPDATED 2025-10-11)
                // NEW: total_fees + 0.5% of gross profit (user requirement)
                // OLD: total_fees * 1.2 (20% margin - too conservative)
                let required_margin = 0.005 * gross_profit; // 0.5% of gross as safety margin
                let min_acceptable = total_fees + required_margin;

                // Check if profitable with required margin and realistic
                // Cap at 5% to avoid fake/manipulated spreads (real arbs are 0.5-3%)
                if net_profit >= min_acceptable && profit_pct < 5.0 && gross_profit > 0.0 {
                    debug!(
                        "✅ Triangle profitable: Gross={:.6} SOL, Fees={:.6} SOL, Net={:.6} SOL, Min Required={:.6} SOL (fees + 0.5% gross)",
                        gross_profit, total_fees, net_profit, min_acceptable
                    );

                    return Some(SimpleTriangleOpportunity {
                        token_a_mint: token_a_mint.to_string(),
                        token_b_mint: token_b_mint.to_string(),
                        dex_1: price_a.dex.clone(),
                        dex_2: "Inferred".to_string(), // We don't know actual A→B DEX
                        dex_3: price_b.dex.clone(),

                        // GHOST POOL FIX: Copy full addresses from ShredStream data
                        pool_1_address: price_a.pool_address.clone(), // Full 44-char address
                        pool_3_address: price_b.pool_address.clone(), // Full 44-char address

                        profit_sol: net_profit, // Store NET profit (after all fees)
                        profit_percentage: profit_pct,
                        input_amount_sol: capital_sol,
                    });
                }
            }
        }

        None
    }
}
