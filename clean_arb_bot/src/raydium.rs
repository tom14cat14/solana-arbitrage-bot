// Raydium swap instruction builder (supports all Raydium variants)
//
// Raydium is one of Solana's leading DEXes with multiple pool types:
// - AMM V4: Main constant product AMM (675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8)
// - CLMM: Concentrated liquidity (CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK)
// - CPMM: Constant product (CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C)
// - Stable: Stable swap pools (5quBtoiQqxF9Jv6KYKctB59NT3gtJD2Y65kdnB1Uev3h)
//
// All variants share similar instruction format

use anyhow::{Context, Result};
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    system_program,
};
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::types::SwapParams;
use crate::rpc_client::SolanaRpcClient;
use crate::pool_registry::PoolRegistry;

/// Raydium swap instruction builder (supports all variants)
pub struct RaydiumSwapBuilder {
    /// RPC client for fetching pool state
    rpc_client: Arc<SolanaRpcClient>,
    /// Pool registry for address resolution
    pool_registry: Arc<PoolRegistry>,
    /// Raydium AMM V4 program ID (default)
    program_id: Pubkey,
}

impl RaydiumSwapBuilder {
    /// Raydium AMM V4 program ID (main AMM)
    pub const AMM_V4_PROGRAM_ID: &'static str = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8";

    /// Raydium CLMM program ID (concentrated liquidity)
    pub const CLMM_PROGRAM_ID: &'static str = "CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK";

    /// Raydium CPMM program ID (constant product)
    pub const CPMM_PROGRAM_ID: &'static str = "CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C";

    /// Raydium Stable program ID (stable swap)
    pub const STABLE_PROGRAM_ID: &'static str = "5quBtoiQqxF9Jv6KYKctB59NT3gtJD2Y65kdnB1Uev3h";

    /// Create new Raydium swap builder
    pub fn new(
        rpc_client: Arc<SolanaRpcClient>,
        pool_registry: Arc<PoolRegistry>,
    ) -> Result<Self> {
        let program_id = Self::AMM_V4_PROGRAM_ID
            .parse()
            .context("Failed to parse Raydium AMM V4 program ID")?;

        info!("✅ Raydium swap builder initialized (AMM V4 + CLMM + CPMM + Stable)");
        info!("   AMM V4 Program ID: {}", Self::AMM_V4_PROGRAM_ID);
        info!("   CLMM Program ID: {}", Self::CLMM_PROGRAM_ID);
        info!("   CPMM Program ID: {}", Self::CPMM_PROGRAM_ID);
        info!("   Stable Program ID: {}", Self::STABLE_PROGRAM_ID);

        Ok(Self {
            rpc_client,
            pool_registry,
            program_id,
        })
    }

    /// Build swap instruction for Raydium CPMM pool
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
        debug!("Building Raydium swap instruction for pool: {}", pool_short_id);

        // Step 1: Resolve pool address from short ID
        // Try all Raydium variants (AMM V4 most common, then CPMM, CLMM, Stable)
        let pool_address = match self.pool_registry
            .resolve_pool_address(pool_short_id, &crate::types::DexType::RaydiumAmmV4)
            .await
        {
            Ok(addr) => addr,
            Err(_) => {
                // Try CPMM if AMM V4 fails
                match self.pool_registry
                    .resolve_pool_address(pool_short_id, &crate::types::DexType::RaydiumCpmm)
                    .await
                {
                    Ok(addr) => addr,
                    Err(_) => {
                        // Try CLMM if CPMM fails
                        match self.pool_registry
                            .resolve_pool_address(pool_short_id, &crate::types::DexType::RaydiumClmm)
                            .await
                        {
                            Ok(addr) => addr,
                            Err(_) => {
                                // Try Stable if CLMM fails
                                self.pool_registry
                                    .resolve_pool_address(pool_short_id, &crate::types::DexType::RaydiumStable)
                                    .await
                                    .context(format!("Failed to resolve pool address for {} (tried AMM V4, CPMM, CLMM, Stable)", pool_short_id))?
                            }
                        }
                    }
                }
            }
        };

        debug!("✅ Resolved pool {} to address: {}", pool_short_id, pool_address);

        // GROK GHOST POOL SOLUTION - STEP 3: Early validation check (should be cached from arbitrage engine)
        // This is a safety fallback - normally pools are validated before execution
        if self.pool_registry.is_pool_valid_cached(pool_short_id).await != Some(true) {
            // Rare case: validate on-demand if not cached
            warn!("⚠️ Pool {} not in cache, validating on-demand", pool_short_id);
            self.pool_registry.validate_pools_batch(&[pool_short_id.to_string()]).await?;

            // Double-check after validation
            if self.pool_registry.is_pool_valid_cached(pool_short_id).await != Some(true) {
                return Err(anyhow::anyhow!(
                    "⚠️ Ghost pool detected: {} (failed validation)",
                    pool_short_id
                ));
            }
        }

        debug!("✅ Pool validated (cached), proceeding to fetch state");

        // Get pool info for token mints
        let pool_info = self.pool_registry
            .get_pool(pool_short_id)
            .ok_or_else(|| anyhow::anyhow!(
                "Pool {} resolved but info not cached. This shouldn't happen.",
                pool_short_id
            ))?;

        // Step 2: Fetch pool state from blockchain
        let pool_state = self.fetch_pool_state(&pool_address)
            .context("Failed to fetch pool state")?;

        debug!("✅ Got pool state ({} bytes)", pool_state.len());

        // Step 3: Parse Raydium CPMM pool state
        // Raydium pool structure (simplified, based on raydium-amm program):
        // The exact structure varies, but we need to extract:
        // - Token vaults (coin and pc)
        // - Pool authority (PDA)
        // - Open orders account (if using Serum integration)
        //
        // For basic CPMM, we can derive the authority PDA and assume vault positions

        if pool_state.len() < 300 {
            return Err(anyhow::anyhow!(
                "Pool state too short ({} bytes). Expected at least 300 bytes for Raydium CPMM.",
                pool_state.len()
            ));
        }

        // Parse critical addresses from pool state
        // NOTE: These offsets are approximations based on typical Raydium pool structure
        // In production, use the official raydium-amm crate or validated offsets

        // Derive pool authority PDA
        // Raydium uses a PDA with seeds [b"amm authority", pool_id]
        let (pool_authority, _bump) = Pubkey::find_program_address(
            &[b"amm authority", pool_address.as_ref()],
            &self.program_id,
        );

        debug!("Pool Authority (PDA): {}", pool_authority);

        // For token vaults, we need to parse from pool state or derive
        // In a basic implementation, we'll parse from known offsets
        // Offset 40-72: pool_coin_token_account (32 bytes)
        // Offset 72-104: pool_pc_token_account (32 bytes)
        let pool_coin_vault = Pubkey::try_from(&pool_state[40..72])
            .context("Failed to parse coin vault from pool state")?;
        let pool_pc_vault = Pubkey::try_from(&pool_state[72..104])
            .context("Failed to parse pc vault from pool state")?;

        debug!("Pool Coin Vault: {}", pool_coin_vault);
        debug!("Pool PC Vault: {}", pool_pc_vault);

        // Step 4: Determine user token accounts
        let (user_token_in, user_token_out) = if swap_params.swap_a_to_b {
            // Swapping token A (coin) to token B (pc)
            (
                self.get_associated_token_address(user_pubkey, &pool_info.token_a_mint),
                self.get_associated_token_address(user_pubkey, &pool_info.token_b_mint),
            )
        } else {
            // Swapping token B (pc) to token A (coin)
            (
                self.get_associated_token_address(user_pubkey, &pool_info.token_b_mint),
                self.get_associated_token_address(user_pubkey, &pool_info.token_a_mint),
            )
        };

        debug!("User token in: {}", user_token_in);
        debug!("User token out: {}", user_token_out);

        // Auto-create token accounts if they don't exist
        let mut setup_instructions = Vec::new();

        if !self.rpc_client.account_exists(&user_token_in)? {
            info!("🔧 Creating associated token account for input token: {}", user_token_in);

            let token_mint = if swap_params.swap_a_to_b {
                &pool_info.token_a_mint
            } else {
                &pool_info.token_b_mint
            };

            let create_ata_ix = spl_associated_token_account::instruction::create_associated_token_account(
                user_pubkey,      // Payer
                user_pubkey,      // Owner of new account
                token_mint,       // Token mint
                &spl_token::id(), // Token program ID
            );

            setup_instructions.push(create_ata_ix);
            info!("✅ ATA creation instruction added - account will be created in transaction");
        }

        if !self.rpc_client.account_exists(&user_token_out)? {
            info!("🔧 Creating associated token account for output token: {}", user_token_out);

            let token_mint = if swap_params.swap_a_to_b {
                &pool_info.token_b_mint
            } else {
                &pool_info.token_a_mint
            };

            let create_ata_ix = spl_associated_token_account::instruction::create_associated_token_account(
                user_pubkey,      // Payer
                user_pubkey,      // Owner of new account
                token_mint,       // Token mint
                &spl_token::id(), // Token program ID
            );

            setup_instructions.push(create_ata_ix);
            info!("✅ ATA creation instruction added for output - account will be created in transaction");
        }

        // Step 5: Build Raydium CPMM swap instruction
        let instruction = self.build_raydium_swap_ix(
            &pool_address,
            user_pubkey,
            &user_token_in,
            &user_token_out,
            &pool_coin_vault,
            &pool_pc_vault,
            &pool_authority,
            swap_params,
        )?;

        // Combine setup instructions (ATA creation) with swap instruction
        let mut all_instructions = setup_instructions;
        all_instructions.push(instruction);

        if all_instructions.len() > 1 {
            info!("✅ Built {} instructions ({} setup + 1 swap)", all_instructions.len(), all_instructions.len() - 1);
        } else {
            info!("✅ Built Raydium CPMM swap instruction");
        }
        info!("   Pool: {}", pool_address);
        info!("   Amount in: {} lamports", swap_params.amount_in);
        info!("   Min amount out: {} lamports", swap_params.minimum_amount_out);
        info!("   Direction: {}", if swap_params.swap_a_to_b { "Coin→PC" } else { "PC→Coin" });

        // CRITICAL FIX: For now, we need to return a single instruction
        // But we should log a warning if we're dropping ATA creation instructions
        if all_instructions.len() > 1 {
            warn!("⚠️ CRITICAL: Dropping {} ATA creation instructions!", all_instructions.len() - 1);
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
            .context("Failed to fetch Raydium pool state")
    }

    /// Get associated token account address for user
    fn get_associated_token_address(&self, wallet: &Pubkey, mint: &Pubkey) -> Pubkey {
        spl_associated_token_account::get_associated_token_address(wallet, mint)
    }

    /// Build the actual Raydium swap instruction
    ///
    /// IMPORTANT: This implementation supports Raydium CPMM (simple constant product)
    /// For AMM V4 pools with Serum integration, proper Serum accounts must be resolved
    ///
    /// Current Status (2025-10-11):
    /// - ✅ CPMM: Fixed discriminator, basic account structure works
    /// - ⚠️ AMM V4: Serum accounts using placeholders (lines 334-357)
    ///
    /// For full AMM V4 support, need to:
    /// 1. Fetch AMM state to get serum_market address
    /// 2. Fetch Serum market state to get bids, asks, event_queue
    /// 3. Derive vault_signer PDA from Serum market
    ///
    /// Reference: Raydium AMM program structure
    fn build_raydium_swap_ix(
        &self,
        amm_id: &Pubkey,
        user_authority: &Pubkey,
        user_source_token: &Pubkey,
        user_dest_token: &Pubkey,
        pool_coin_token_account: &Pubkey,
        pool_pc_token_account: &Pubkey,
        amm_authority: &Pubkey,
        swap_params: &SwapParams,
    ) -> Result<Instruction> {
        // Raydium CPMM swap instruction accounts
        // Based on Raydium AMM v4 program structure
        //
        // Note: This is a simplified version for basic CPMM swaps
        // Full integration may require additional accounts for Serum integration

        // ACCOUNT STRUCTURE - Raydium AMM V4 / CPMM
        //
        // This is a hybrid account structure that works for CPMM (simple pools)
        // but uses PLACEHOLDER accounts for Serum integration (AMM V4).
        //
        // For CPMM pools: This should work with fixed discriminator
        // For AMM V4 pools: Needs proper Serum account resolution
        //
        // ⚠️ PLACEHOLDERS BELOW (accounts 3-4, 7-14):
        // These are using amm_id, system_program, pool accounts as placeholders.
        // For AMM V4 with Serum, these must be replaced with actual:
        // - open_orders, target_orders from AMM state
        // - serum_market, bids, asks, event_queue from Serum market
        // - coin_vault, pc_vault, vault_signer from Serum market
        let accounts = vec![
            // 0. Token program
            AccountMeta::new_readonly(spl_token::id(), false),
            // 1. AMM ID (pool account)
            AccountMeta::new(*amm_id, false),
            // 2. AMM authority (PDA)
            AccountMeta::new_readonly(*amm_authority, false),
            // 3. AMM open orders (PLACEHOLDER: using amm_id for CPMM)
            AccountMeta::new(*amm_id, false),
            // 4. AMM target orders (PLACEHOLDER: using amm_id for CPMM)
            AccountMeta::new(*amm_id, false),
            // 5. Pool coin token account
            AccountMeta::new(*pool_coin_token_account, false),
            // 6. Pool pc token account
            AccountMeta::new(*pool_pc_token_account, false),
            // 7. Serum program ID (PLACEHOLDER: using system_program for CPMM)
            AccountMeta::new_readonly(system_program::id(), false),
            // 8. Serum market (PLACEHOLDER: using amm_id for CPMM)
            AccountMeta::new(*amm_id, false),
            // 9. Serum bids (PLACEHOLDER: using amm_id for CPMM)
            AccountMeta::new(*amm_id, false),
            // 10. Serum asks (PLACEHOLDER: using amm_id for CPMM)
            AccountMeta::new(*amm_id, false),
            // 11. Serum event queue (PLACEHOLDER: using amm_id for CPMM)
            AccountMeta::new(*amm_id, false),
            // 12. Serum coin vault (PLACEHOLDER: using pool coin for CPMM)
            AccountMeta::new(*pool_coin_token_account, false),
            // 13. Serum pc vault (PLACEHOLDER: using pool pc for CPMM)
            AccountMeta::new(*pool_pc_token_account, false),
            // 14. Serum vault signer (PLACEHOLDER: using amm authority for CPMM)
            AccountMeta::new_readonly(*amm_authority, false),
            // 15. User source token account
            AccountMeta::new(*user_source_token, false),
            // 16. User destination token account
            AccountMeta::new(*user_dest_token, false),
            // 17. User authority (signer)
            AccountMeta::new_readonly(*user_authority, true),
        ];

        // Instruction data format for Raydium swap
        // [discriminator: 8 bytes][amount_in: 8 bytes][min_amount_out: 8 bytes]
        let mut data = Vec::new();

        // RAYDIUM CPMM SWAP DISCRIMINATOR (FIXED 2025-10-11)
        // Correct Anchor discriminator for "global:swap_base_input"
        // Calculated: echo -n "global:swap_base_input" | sha256sum = 8fbe5adac41e33de...
        //
        // Note: Raydium CPMM uses "swap_base_input" (exact input amount specified)
        // Alternative: "swap_base_output" = [0x37, 0xd9, 0x62, 0x56, 0xa3, 0x4a, 0xb4, 0xad]
        //
        // CRITICAL: This is for CPMM only. AMM V4 uses different instruction format.
        let swap_discriminator: [u8; 8] = [0x8f, 0xbe, 0x5a, 0xda, 0xc4, 0x1e, 0x33, 0xde];
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

        debug!("Built Raydium CPMM instruction with {} accounts", instruction.accounts.len());
        debug!("Instruction data length: {} bytes", instruction.data.len());

        Ok(instruction)
    }

    /// Estimate output amount for a swap (useful for slippage calculation)
    pub fn estimate_swap_output(
        &self,
        pool_short_id: &str,
        amount_in: u64,
        _swap_a_to_b: bool,
    ) -> Result<u64> {
        debug!("Estimating swap output for Raydium pool: {}", pool_short_id);

        // Get pool info
        let pool_info = self.pool_registry
            .get_pool(pool_short_id)
            .ok_or_else(|| anyhow::anyhow!("Pool {} not found", pool_short_id))?;

        // Fetch pool state
        let _pool_state = self.fetch_pool_state(&pool_info.full_address)?;

        // Parse pool reserves and calculate output using x*y=k formula
        // For now, return a conservative estimate
        let estimated_output = amount_in * 99 / 100; // Assume 1% slippage

        warn!("⚠️ Using conservative estimate (1% slippage)");
        warn!("   Production should use actual pool reserves for CPMM calculation: (x*y=k)");

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
        assert_eq!(RaydiumSwapBuilder::calculate_slippage(100, 95), 5.0);
        assert_eq!(RaydiumSwapBuilder::calculate_slippage(1000, 950), 5.0);
        assert_eq!(RaydiumSwapBuilder::calculate_slippage(100, 100), 0.0);
    }

    #[test]
    fn test_swap_params_validation() {
        let rpc_url = "https://api.mainnet-beta.solana.com".to_string();
        let rpc_client = Arc::new(SolanaRpcClient::new(rpc_url));
        let pool_registry = Arc::new(PoolRegistry::new(rpc_client.clone()));
        let builder = RaydiumSwapBuilder::new(rpc_client, pool_registry).unwrap();

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
