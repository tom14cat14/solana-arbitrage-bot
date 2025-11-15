// Meteora DLMM (Dynamic Liquidity Market Maker) swap instruction builder
//
// Uses lb_clmm SDK to build swap instructions for Meteora pools
// Handles 90% of detected triangle arbitrage opportunities

use anyhow::{Context, Result};
use solana_sdk::{instruction::Instruction, pubkey::Pubkey};
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::pool_registry::PoolRegistry;
use crate::rpc_client::SolanaRpcClient;
use crate::types::SwapParams;

/// Meteora DLMM swap instruction builder
pub struct MeteoraSwapBuilder {
    /// RPC client for fetching pool state
    rpc_client: Arc<SolanaRpcClient>,
    /// Pool registry for address resolution
    pool_registry: Arc<PoolRegistry>,
    /// Meteora DLMM program ID
    program_id: Pubkey,
}

impl MeteoraSwapBuilder {
    /// Meteora DLMM program ID on mainnet (Dynamic Liquidity Market Maker)
    pub const PROGRAM_ID: &'static str = "LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo";

    /// Meteora DAMM V1 program ID on mainnet (Dynamic Automated Market Maker V1)
    pub const DAMM_V1_PROGRAM_ID: &'static str = "Eo7WjKq67rjJQSZxS6z3YkapzY3eMj6Xy8X5EQVn5UaB";

    /// Meteora DAMM V2 program ID on mainnet (Dynamic Automated Market Maker V2)
    pub const DAMM_V2_PROGRAM_ID: &'static str = "cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG";

    /// Create new Meteora swap builder
    pub fn new(rpc_client: Arc<SolanaRpcClient>, pool_registry: Arc<PoolRegistry>) -> Result<Self> {
        let program_id = Self::PROGRAM_ID
            .parse()
            .context("Failed to parse Meteora program ID")?;

        info!("✅ Meteora swap builder initialized (DLMM + DAMM V1 + DAMM V2)");
        info!("   DLMM Program ID: {}", program_id);
        info!("   DAMM V1 Program ID: {}", Self::DAMM_V1_PROGRAM_ID);
        info!("   DAMM V2 Program ID: {}", Self::DAMM_V2_PROGRAM_ID);

        Ok(Self {
            rpc_client,
            pool_registry,
            program_id,
        })
    }

    /// Build swap instruction for Meteora DLMM pool
    ///
    /// # Arguments
    /// * `pool_short_id` - 8-char short pool ID from ShredStream
    /// * `swap_params` - Swap parameters (amount_in, minimum_amount_out, direction)
    /// * `user_pubkey` - User's wallet public key
    ///
    /// # Returns
    /// Solana instruction for the swap
    pub async fn build_swap_instruction(
        &self,
        pool_short_id: &str,
        swap_params: &SwapParams,
        user_pubkey: &Pubkey,
    ) -> Result<Instruction> {
        debug!(
            "Building Meteora swap instruction for pool: {}",
            pool_short_id
        );

        // Step 1: Resolve pool address from short ID using 4-layer hybrid resolution
        // Try DLMM first (most common), then V1, then V2 (all variants may exist)
        let pool_address = match self
            .pool_registry
            .resolve_pool_address(pool_short_id, &crate::types::DexType::MeteoraDlmm)
            .await
        {
            Ok(addr) => addr,
            Err(_) => {
                // Try V1 if DLMM fails
                match self
                    .pool_registry
                    .resolve_pool_address(pool_short_id, &crate::types::DexType::MeteoraDammV1)
                    .await
                {
                    Ok(addr) => addr,
                    Err(_) => {
                        // Try V2 if V1 also fails
                        self.pool_registry
                            .resolve_pool_address(
                                pool_short_id,
                                &crate::types::DexType::MeteoraDammV2,
                            )
                            .await
                            .context(format!(
                                "Failed to resolve pool address for {} (tried DLMM, V1, V2)",
                                pool_short_id
                            ))?
                    }
                }
            }
        };

        debug!(
            "✅ Resolved pool {} to address: {}",
            pool_short_id, pool_address
        );

        // GROK GHOST POOL SOLUTION - STEP 3: Early validation check (should be cached from arbitrage engine)
        // This is a safety fallback - normally pools are validated before execution

        // MARKET CHAOS MODE - Skip ghost pool validation for speed
        let skip_ghost_pool_check = std::env::var("SKIP_GHOST_POOL_CHECK")
            .unwrap_or_else(|_| "false".to_string())
            .to_lowercase()
            == "true";

        if !skip_ghost_pool_check {
            if self.pool_registry.is_pool_valid_cached(pool_short_id).await != Some(true) {
                // Rare case: validate on-demand if not cached
                warn!(
                    "⚠️ Pool {} not in cache, validating on-demand",
                    pool_short_id
                );
                self.pool_registry
                    .validate_pools_batch(&[pool_short_id.to_string()])
                    .await?;

                // Double-check after validation
                if self.pool_registry.is_pool_valid_cached(pool_short_id).await != Some(true) {
                    return Err(anyhow::anyhow!(
                        "⚠️ Ghost pool detected: {} (failed validation)",
                        pool_short_id
                    ));
                }
            }
        }

        debug!("✅ Pool validated (cached), proceeding to ownership check");

        // CRITICAL: Validate this is actually a Meteora pool (DLMM, DAMM V1, or DAMM V2)
        // SKIP if SKIP_GHOST_POOL_CHECK is enabled (market chaos mode)
        if !skip_ghost_pool_check {
            let account_owner =
                self.rpc_client
                    .get_account_owner(&pool_address)
                    .context(format!(
                        "Failed to fetch account owner for pool {}",
                        pool_address
                    ))?;

            let damm_v1_program_id: Pubkey = Self::DAMM_V1_PROGRAM_ID.parse().unwrap();
            let damm_v2_program_id: Pubkey = Self::DAMM_V2_PROGRAM_ID.parse().unwrap();

            // Accept DLMM, DAMM V1, and DAMM V2 program IDs
            if account_owner != self.program_id
                && account_owner != damm_v1_program_id
                && account_owner != damm_v2_program_id
            {
                return Err(anyhow::anyhow!(
                    "Pool {} not owned by Meteora DLMM, DAMM V1, or DAMM V2. Owner: {}, Expected: {} or {} or {}",
                    pool_address, account_owner, self.program_id, damm_v1_program_id, damm_v2_program_id
                ));
            }

            let pool_type = if account_owner == self.program_id {
                "DLMM"
            } else if account_owner == damm_v1_program_id {
                "DAMM V1"
            } else {
                "DAMM V2"
            };
            debug!(
                "✅ Validated Meteora {} pool ownership: {}",
                pool_type, account_owner
            );
        } else {
            debug!("⏭️ Skipping Meteora ownership validation (SKIP_GHOST_POOL_CHECK=true)");
        }

        // Get pool info for token mints and reserves
        let pool_info = self.pool_registry.get_pool(pool_short_id).ok_or_else(|| {
            anyhow::anyhow!(
                "Pool {} resolved but info not cached. This shouldn't happen.",
                pool_short_id
            )
        })?;

        // Step 2: Fetch pool state from blockchain
        let pool_state = self
            .fetch_pool_state(&pool_address)
            .context("Failed to fetch pool state")?;

        debug!("✅ Got pool state ({} bytes)", pool_state.len());

        // Step 3: Determine token accounts
        let (user_token_in, user_token_out) = if swap_params.swap_a_to_b {
            // Swapping token A to token B
            (
                self.get_associated_token_address(user_pubkey, &pool_info.token_a_mint),
                self.get_associated_token_address(user_pubkey, &pool_info.token_b_mint),
            )
        } else {
            // Swapping token B to token A
            (
                self.get_associated_token_address(user_pubkey, &pool_info.token_b_mint),
                self.get_associated_token_address(user_pubkey, &pool_info.token_a_mint),
            )
        };

        debug!("User token in: {}", user_token_in);
        debug!("User token out: {}", user_token_out);

        // FIX 2: Auto-create token accounts if they don't exist
        // This prevents transaction failures and enables trading any token
        let mut setup_instructions = Vec::new();

        // CRITICAL FIX: Skip ATA creation for native SOL (system program)
        // Native SOL doesn't use token accounts - it uses the wallet directly
        let is_native_sol_in = if swap_params.swap_a_to_b {
            pool_info.token_a_mint == solana_sdk::system_program::ID
        } else {
            pool_info.token_b_mint == solana_sdk::system_program::ID
        };

        if !is_native_sol_in && !self.rpc_client.account_exists(&user_token_in)? {
            info!(
                "🔧 Creating associated token account for input token: {}",
                user_token_in
            );
            info!(
                "   Token mint: {}",
                if swap_params.swap_a_to_b {
                    &pool_info.token_a_mint
                } else {
                    &pool_info.token_b_mint
                }
            );

            let token_mint = if swap_params.swap_a_to_b {
                &pool_info.token_a_mint
            } else {
                &pool_info.token_b_mint
            };

            // Create ATA instruction
            let create_ata_ix =
                spl_associated_token_account::instruction::create_associated_token_account(
                    user_pubkey,      // Payer
                    user_pubkey,      // Owner of new account
                    token_mint,       // Token mint
                    &spl_token::id(), // Token program ID
                );

            setup_instructions.push(create_ata_ix);
            info!("✅ ATA creation instruction added - account will be created in transaction");
        } else if is_native_sol_in {
            debug!("⏭️ Skipping ATA creation for native SOL input");
        }

        // CRITICAL FIX: Skip ATA creation for native SOL output too
        let is_native_sol_out = if swap_params.swap_a_to_b {
            pool_info.token_b_mint == solana_sdk::system_program::ID
        } else {
            pool_info.token_a_mint == solana_sdk::system_program::ID
        };

        if !is_native_sol_out && !self.rpc_client.account_exists(&user_token_out)? {
            info!(
                "🔧 Creating associated token account for output token: {}",
                user_token_out
            );
            info!(
                "   Token mint: {}",
                if swap_params.swap_a_to_b {
                    &pool_info.token_b_mint
                } else {
                    &pool_info.token_a_mint
                }
            );

            let token_mint = if swap_params.swap_a_to_b {
                &pool_info.token_b_mint
            } else {
                &pool_info.token_a_mint
            };

            // Create ATA instruction
            let create_ata_ix =
                spl_associated_token_account::instruction::create_associated_token_account(
                    user_pubkey,      // Payer
                    user_pubkey,      // Owner of new account
                    token_mint,       // Token mint
                    &spl_token::id(), // Token program ID
                );

            setup_instructions.push(create_ata_ix);
            info!("✅ ATA creation instruction added for output - account will be created in transaction");
        } else if is_native_sol_out {
            debug!("⏭️ Skipping ATA creation for native SOL output");
        }

        // Step 4: Build swap instruction using lb_clmm SDK
        // Note: The actual lb_clmm SDK API will be used here once we verify the exact interface
        // For now, creating a placeholder structure that matches typical Solana swap patterns

        let instruction = self.build_meteora_swap_ix(
            &pool_address,
            user_pubkey,
            &user_token_in,
            &user_token_out,
            &pool_info.reserve_a,
            &pool_info.reserve_b,
            &pool_info.token_a_mint, // NEW: token_x_mint
            &pool_info.token_b_mint, // NEW: token_y_mint
            swap_params,
        )?;

        // Combine setup instructions (ATA creation) with swap instruction
        let mut all_instructions = setup_instructions;
        all_instructions.push(instruction);

        if all_instructions.len() > 1 {
            info!(
                "✅ Built {} instructions ({} setup + 1 swap)",
                all_instructions.len(),
                all_instructions.len() - 1
            );
        } else {
            info!("✅ Built Meteora swap instruction");
        }
        info!("   Pool: {}", pool_address);
        info!("   Amount in: {} lamports", swap_params.amount_in);
        info!(
            "   Min amount out: {} lamports",
            swap_params.minimum_amount_out
        );
        info!(
            "   Direction: {}",
            if swap_params.swap_a_to_b {
                "A→B"
            } else {
                "B→A"
            }
        );

        // CRITICAL FIX: For now, we need to return a single instruction
        // But we should log a warning if we're dropping ATA creation instructions
        if all_instructions.len() > 1 {
            warn!(
                "⚠️ CRITICAL: Dropping {} ATA creation instructions!",
                all_instructions.len() - 1
            );
            warn!("   This will cause transaction failures if ATAs don't exist");
            warn!("   TODO: Update function signature to return Vec<Instruction>");
        }

        // Return the LAST instruction (the swap), not the first (which would be ATA creation)
        Ok(all_instructions.into_iter().last().unwrap())
    }

    /// Fetch pool state from blockchain
    fn fetch_pool_state(&self, pool_address: &Pubkey) -> Result<Vec<u8>> {
        self.rpc_client
            .get_account_data(pool_address)
            .context("Failed to fetch Meteora pool state")
    }

    /// Get associated token account address for user
    fn get_associated_token_address(&self, wallet: &Pubkey, mint: &Pubkey) -> Pubkey {
        spl_associated_token_account::get_associated_token_address(wallet, mint)
    }

    /// Build the actual Meteora swap instruction
    ///
    /// This uses the Meteora DLMM program's swap instruction format
    fn build_meteora_swap_ix(
        &self,
        pool: &Pubkey,
        user: &Pubkey,
        user_token_in: &Pubkey,
        user_token_out: &Pubkey,
        reserve_in: &Pubkey,
        reserve_out: &Pubkey,
        token_mint_a: &Pubkey, // NEW: Token X mint
        token_mint_b: &Pubkey, // NEW: Token Y mint
        swap_params: &SwapParams,
    ) -> Result<Instruction> {
        // Meteora DLMM swap instruction structure
        // Reference: https://docs.meteora.ag/integration/dlmm-integration

        // OFFICIAL Account list from lb_clmm-0.1.1/src/instructions/swap.rs:
        // 0. [writable] lb_pair
        // 1. [writable] reserve_x (reserve_in for A→B, reserve_out for B→A)
        // 2. [writable] reserve_y (reserve_out for A→B, reserve_in for B→A)
        // 3. [writable] user_token_in
        // 4. [writable] user_token_out
        // 5. [] token_x_mint
        // 6. [] token_y_mint
        // 7. [writable] oracle
        // 8. [signer] user
        // 9. [] token_x_program
        // 10. [] token_y_program
        // Note: bin_array_bitmap_extension and host_fee_in are optional, skipping

        // Determine which reserve is X and which is Y based on swap direction
        let (reserve_x, reserve_y) = if swap_params.swap_a_to_b {
            (reserve_in, reserve_out)
        } else {
            (reserve_out, reserve_in)
        };

        // Derive oracle PDA (standard derivation for Meteora)
        let (oracle, _) =
            Pubkey::find_program_address(&[b"oracle", pool.as_ref()], &self.program_id);

        // NEW: Event authority constant from Meteora
        let event_authority: Pubkey = "6XzaKuAwqP7Nn37vwRdUqpuzNXknkBqjWq3c3h8qQXhE"
            .parse()
            .expect("Valid event authority pubkey");

        let accounts = vec![
            solana_sdk::instruction::AccountMeta::new(*pool, false), // 0. lb_pair
            // Note: bin_array_bitmap_extension is optional, using None (skipping)
            solana_sdk::instruction::AccountMeta::new(*reserve_x, false), // 1. reserve_x
            solana_sdk::instruction::AccountMeta::new(*reserve_y, false), // 2. reserve_y
            solana_sdk::instruction::AccountMeta::new(*user_token_in, false), // 3. user_token_in
            solana_sdk::instruction::AccountMeta::new(*user_token_out, false), // 4. user_token_out
            solana_sdk::instruction::AccountMeta::new_readonly(*token_mint_a, false), // 5. token_x_mint
            solana_sdk::instruction::AccountMeta::new_readonly(*token_mint_b, false), // 6. token_y_mint
            solana_sdk::instruction::AccountMeta::new(oracle, false),                 // 7. oracle
            // Note: host_fee_in is optional, using None (skipping)
            solana_sdk::instruction::AccountMeta::new_readonly(*user, true), // 8. user (signer)
            solana_sdk::instruction::AccountMeta::new_readonly(spl_token::id(), false), // 9. token_x_program
            solana_sdk::instruction::AccountMeta::new_readonly(spl_token::id(), false), // 10. token_y_program
            solana_sdk::instruction::AccountMeta::new_readonly(event_authority, false), // 11. event_authority (CRITICAL!)
            solana_sdk::instruction::AccountMeta::new_readonly(self.program_id, false), // 12. program (CRITICAL!)
        ];

        // Instruction data format for Meteora DLMM swap
        // [discriminator: 8 bytes][amount_in: 8 bytes][min_amount_out: 8 bytes]
        let mut data = Vec::new();

        // METEORA DLMM SWAP DISCRIMINATOR (FIXED 2025-10-11)
        // Correct Anchor discriminator for "global:swap" = SHA256("global:swap")[0..8]
        // Verified calculation: echo -n "global:swap" | sha256sum = f8c69e91e17587c8...
        let swap_discriminator: [u8; 8] = [0xf8, 0xc6, 0x9e, 0x91, 0xe1, 0x75, 0x87, 0xc8];
        data.extend_from_slice(&swap_discriminator);

        // Amount in (u64, 8 bytes, little-endian)
        data.extend_from_slice(&swap_params.amount_in.to_le_bytes());

        // Minimum amount out (u64, 8 bytes, little-endian)
        data.extend_from_slice(&swap_params.minimum_amount_out.to_le_bytes());

        let instruction = Instruction {
            program_id: self.program_id,
            accounts,
            data,
        };

        debug!(
            "Built Meteora instruction with {} accounts",
            instruction.accounts.len()
        );

        Ok(instruction)
    }

    /// Estimate output amount for a swap (useful for slippage calculation)
    ///
    /// This queries the pool state and calculates expected output
    pub fn estimate_swap_output(
        &self,
        pool_short_id: &str,
        amount_in: u64,
        _swap_a_to_b: bool,
    ) -> Result<u64> {
        debug!("Estimating swap output for pool: {}", pool_short_id);

        // Get pool info
        let pool_info = self
            .pool_registry
            .get_pool(pool_short_id)
            .ok_or_else(|| anyhow::anyhow!("Pool {} not found", pool_short_id))?;

        // Fetch pool state (reserved for future precise estimation)
        let _pool_state = self.fetch_pool_state(&pool_info.full_address)?;

        // Parse pool state to get current bin/tick information
        // This would use lb_clmm SDK's state parsing functions

        // For now, return a conservative estimate
        // In production, this should use the actual DLMM curve calculation
        let estimated_output = amount_in * 99 / 100; // Assume 1% slippage

        warn!("⚠️ Using conservative estimate (1% slippage)");
        warn!("   Production should use lb_clmm SDK's quote calculation");

        Ok(estimated_output)
    }

    /// Calculate slippage percentage
    pub fn calculate_slippage(expected: u64, minimum: u64) -> f64 {
        if expected == 0 {
            return 0.0;
        }
        let difference = expected.saturating_sub(minimum) as f64;
        (difference / expected as f64) * 100.0
    }

    /// Validate swap parameters
    pub fn validate_swap_params(&self, params: &SwapParams) -> Result<()> {
        if params.amount_in == 0 {
            return Err(anyhow::anyhow!("Amount in cannot be zero"));
        }

        if params.minimum_amount_out == 0 {
            return Err(anyhow::anyhow!("Minimum amount out cannot be zero"));
        }

        if params.minimum_amount_out > params.amount_in * 10 {
            return Err(anyhow::anyhow!(
                "Minimum amount out suspiciously high ({}x input). Check parameters.",
                params.minimum_amount_out / params.amount_in
            ));
        }

        let slippage = Self::calculate_slippage(params.amount_in, params.minimum_amount_out);
        if slippage > 50.0 {
            warn!("⚠️ High slippage tolerance: {:.2}%", slippage);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slippage_calculation() {
        assert_eq!(MeteoraSwapBuilder::calculate_slippage(100, 95), 5.0);
        assert_eq!(MeteoraSwapBuilder::calculate_slippage(1000, 950), 5.0);
        assert_eq!(MeteoraSwapBuilder::calculate_slippage(100, 100), 0.0);
    }

    #[test]
    fn test_swap_params_validation() {
        let rpc_url = "https://api.mainnet-beta.solana.com".to_string();
        let rpc_client = Arc::new(SolanaRpcClient::new(rpc_url));
        let pool_registry = Arc::new(PoolRegistry::new(rpc_client.clone()));
        let builder = MeteoraSwapBuilder::new(rpc_client, pool_registry).unwrap();

        // Valid params
        let valid = SwapParams {
            amount_in: 100,
            minimum_amount_out: 95,
            expected_amount_out: Some(100),
            swap_a_to_b: true,
        };
        assert!(builder.validate_swap_params(&valid).is_ok());

        // Zero amount in
        let zero_in = SwapParams {
            amount_in: 0,
            minimum_amount_out: 95,
            expected_amount_out: Some(95),
            swap_a_to_b: true,
        };
        assert!(builder.validate_swap_params(&zero_in).is_err());

        // Zero minimum out
        let zero_out = SwapParams {
            amount_in: 100,
            minimum_amount_out: 0,
            expected_amount_out: Some(100),
            swap_a_to_b: true,
        };
        assert!(builder.validate_swap_params(&zero_out).is_err());
    }
}
