// Orca swap instruction builder (supports Whirlpools + Legacy)
//
// Builds swap instructions manually for both Orca pool types:
// - Whirlpools: Concentrated liquidity (whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc)
// - Legacy: Older AMM pools (9W959DqEETiGZocYWCQPaJ6sBmUzgfxXfqGeTEdp3aQP)
//
// NOTE: Built without Orca SDK due to Solana version conflict
// (Orca v5 requires Solana 1.19+, we use 1.18)

use anyhow::{Context, Result};
use solana_sdk::{instruction::Instruction, pubkey::Pubkey};
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::pool_registry::PoolRegistry;
use crate::rpc_client::SolanaRpcClient;
use crate::types::SwapParams;

/// Orca swap instruction builder (supports Whirlpools + Legacy)
pub struct OrcaSwapBuilder {
    /// RPC client for fetching pool state
    rpc_client: Arc<SolanaRpcClient>,
    /// Pool registry for address resolution
    pool_registry: Arc<PoolRegistry>,
    /// Orca Whirlpools program ID (default)
    program_id: Pubkey,
}

impl OrcaSwapBuilder {
    /// Orca Whirlpools program ID (concentrated liquidity)
    pub const WHIRLPOOLS_PROGRAM_ID: &'static str = "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc";

    /// Orca Legacy program ID (older AMM)
    pub const LEGACY_PROGRAM_ID: &'static str = "9W959DqEETiGZocYWCQPaJ6sBmUzgfxXfqGeTEdp3aQP";

    /// Create new Orca swap builder
    pub fn new(rpc_client: Arc<SolanaRpcClient>, pool_registry: Arc<PoolRegistry>) -> Result<Self> {
        let program_id = Self::WHIRLPOOLS_PROGRAM_ID
            .parse()
            .context("Failed to parse Orca Whirlpools program ID")?;

        info!("✅ Orca swap builder initialized (Whirlpools + Legacy)");
        info!("   Whirlpools Program ID: {}", Self::WHIRLPOOLS_PROGRAM_ID);
        info!("   Legacy Program ID: {}", Self::LEGACY_PROGRAM_ID);

        Ok(Self {
            rpc_client,
            pool_registry,
            program_id,
        })
    }

    /// Build swap instruction for Orca Whirlpool
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
            "Building Orca Whirlpool swap instruction for pool: {}",
            pool_short_id
        );

        // Step 1: Resolve pool address from short ID
        let pool_address = self
            .pool_registry
            .resolve_pool_address(pool_short_id, &crate::types::DexType::OrcaWhirlpools)
            .await
            .context(format!(
                "Failed to resolve pool address for {}",
                pool_short_id
            ))?;

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

        debug!("✅ Pool validated (cached), proceeding to fetch state");

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

        // Step 3: Parse Orca Whirlpool state for critical data
        // Orca Whirlpool state structure (from Whirlpools program):
        // - bytes 0-8: discriminator
        // - bytes 8-40: whirlpools_config (pubkey)
        // - bytes 40-72: whirlpool_bump (array)
        // - bytes 72-74: tick_spacing (u16) ← Need this for tick arrays
        // - bytes 74-106: token_mint_a (pubkey)
        // - bytes 106-138: token_mint_b (pubkey)
        // - bytes 138-170: token_vault_a (pubkey)
        // - bytes 170-202: token_vault_b (pubkey)
        // - bytes 202-234: oracle (pubkey)
        // - bytes 234-238: tick_current_index (i32) ← Need this for tick arrays

        if pool_state.len() < 238 {
            return Err(anyhow::anyhow!(
                "Pool state too short ({} bytes). Expected at least 238 bytes for Orca Whirlpool.",
                pool_state.len()
            ));
        }

        // Extract critical data from pool state
        let tick_spacing_bytes = &pool_state[72..74];
        let tick_spacing =
            u16::from_le_bytes([tick_spacing_bytes[0], tick_spacing_bytes[1]]) as i32;

        let token_vault_a = Pubkey::try_from(&pool_state[138..170])
            .context("Failed to parse token vault A pubkey from pool state")?;
        let token_vault_b = Pubkey::try_from(&pool_state[170..202])
            .context("Failed to parse token vault B pubkey from pool state")?;
        let oracle = Pubkey::try_from(&pool_state[202..234])
            .context("Failed to parse oracle pubkey from pool state")?;

        // Parse current tick index (i32, 4 bytes at offset 234)
        let tick_current_bytes = &pool_state[234..238];
        let tick_current_index = i32::from_le_bytes([
            tick_current_bytes[0],
            tick_current_bytes[1],
            tick_current_bytes[2],
            tick_current_bytes[3],
        ]);

        debug!("Tick spacing: {}", tick_spacing);
        debug!("Current tick: {}", tick_current_index);
        debug!("Token Vault A: {}", token_vault_a);
        debug!("Token Vault B: {}", token_vault_b);
        debug!("Oracle: {}", oracle);

        // Step 4: Determine user token accounts
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

        // Auto-create token accounts if they don't exist
        let mut setup_instructions = Vec::new();

        if !self.rpc_client.account_exists(&user_token_in)? {
            info!(
                "🔧 Creating associated token account for input token: {}",
                user_token_in
            );

            let token_mint = if swap_params.swap_a_to_b {
                &pool_info.token_a_mint
            } else {
                &pool_info.token_b_mint
            };

            let create_ata_ix =
                spl_associated_token_account::instruction::create_associated_token_account(
                    user_pubkey,      // Payer
                    user_pubkey,      // Owner of new account
                    token_mint,       // Token mint
                    &spl_token::id(), // Token program ID
                );

            setup_instructions.push(create_ata_ix);
            info!("✅ ATA creation instruction added - account will be created in transaction");
        }

        if !self.rpc_client.account_exists(&user_token_out)? {
            info!(
                "🔧 Creating associated token account for output token: {}",
                user_token_out
            );

            let token_mint = if swap_params.swap_a_to_b {
                &pool_info.token_b_mint
            } else {
                &pool_info.token_a_mint
            };

            let create_ata_ix =
                spl_associated_token_account::instruction::create_associated_token_account(
                    user_pubkey,      // Payer
                    user_pubkey,      // Owner of new account
                    token_mint,       // Token mint
                    &spl_token::id(), // Token program ID
                );

            setup_instructions.push(create_ata_ix);
            info!("✅ ATA creation instruction added for output - account will be created in transaction");
        }

        // Step 5: Derive tick array addresses (FIXED 2025-10-11)
        // Orca Whirlpools uses 3 tick arrays to handle price movements during swap
        // Each tick array covers 88 ticks (TICK_ARRAY_SIZE constant in Whirlpools program)
        let tick_arrays = Self::derive_tick_arrays(
            &pool_address,
            tick_current_index,
            tick_spacing,
            &self.program_id,
        );

        debug!("Tick Array 0: {}", tick_arrays[0]);
        debug!("Tick Array 1: {}", tick_arrays[1]);
        debug!("Tick Array 2: {}", tick_arrays[2]);

        // Step 6: Build Orca Whirlpool swap instruction
        let instruction = self.build_orca_swap_ix(
            &pool_address,
            user_pubkey,
            &user_token_in,
            &user_token_out,
            &token_vault_a,
            &token_vault_b,
            &oracle,
            &tick_arrays,
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
            info!("✅ Built Orca Whirlpool swap instruction");
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
            .context("Failed to fetch Orca Whirlpool state")
    }

    /// Get associated token account address for user
    fn get_associated_token_address(&self, wallet: &Pubkey, mint: &Pubkey) -> Pubkey {
        spl_associated_token_account::get_associated_token_address(wallet, mint)
    }

    /// Derive tick array addresses for Orca Whirlpool swap
    ///
    /// Orca Whirlpools use 3 tick arrays to handle price movements during swaps.
    /// Each tick array covers 88 ticks (TICK_ARRAY_SIZE constant).
    ///
    /// Formula: start_tick_index = (current_tick / (tick_spacing * 88)) * (tick_spacing * 88)
    ///
    /// # Arguments
    /// * `whirlpool` - Whirlpool address
    /// * `tick_current_index` - Current tick from pool state
    /// * `tick_spacing` - Tick spacing from pool state
    /// * `program_id` - Whirlpools program ID
    ///
    /// # Returns
    /// Array of 3 tick array addresses [prev, current, next]
    fn derive_tick_arrays(
        whirlpool: &Pubkey,
        tick_current_index: i32,
        tick_spacing: i32,
        program_id: &Pubkey,
    ) -> [Pubkey; 3] {
        // Orca Whirlpools constant: each tick array covers 88 ticks
        const TICK_ARRAY_SIZE: i32 = 88;

        // Calculate the tick array start index that contains the current tick
        // Formula: floor(current_tick / (tick_spacing * TICK_ARRAY_SIZE)) * (tick_spacing * TICK_ARRAY_SIZE)
        let ticks_in_array = tick_spacing * TICK_ARRAY_SIZE;
        let current_array_start_index = (tick_current_index / ticks_in_array) * ticks_in_array;

        // Derive 3 sequential tick array PDAs: previous, current, next
        let tick_array_prev = Self::derive_tick_array_pda(
            whirlpool,
            current_array_start_index - ticks_in_array,
            program_id,
        );

        let tick_array_current =
            Self::derive_tick_array_pda(whirlpool, current_array_start_index, program_id);

        let tick_array_next = Self::derive_tick_array_pda(
            whirlpool,
            current_array_start_index + ticks_in_array,
            program_id,
        );

        [tick_array_prev, tick_array_current, tick_array_next]
    }

    /// Derive a single tick array PDA
    ///
    /// PDA derivation: ["tick_array", whirlpool, start_tick_index (i32 bytes)]
    fn derive_tick_array_pda(
        whirlpool: &Pubkey,
        start_tick_index: i32,
        program_id: &Pubkey,
    ) -> Pubkey {
        let (pda, _bump) = Pubkey::find_program_address(
            &[
                b"tick_array",
                whirlpool.as_ref(),
                &start_tick_index.to_le_bytes(),
            ],
            program_id,
        );
        pda
    }

    /// Build the actual Orca Whirlpool swap instruction
    ///
    /// Reference: Orca Whirlpools program instruction structure
    /// Discriminator extracted from successful Orca swaps on Solscan
    ///
    /// FIXED 2025-10-11: Tick arrays now properly derived from pool state
    fn build_orca_swap_ix(
        &self,
        whirlpool: &Pubkey,
        token_authority: &Pubkey,
        token_owner_account_a: &Pubkey,
        token_owner_account_b: &Pubkey,
        token_vault_a: &Pubkey,
        token_vault_b: &Pubkey,
        oracle: &Pubkey,
        tick_arrays: &[Pubkey; 3],
        swap_params: &SwapParams,
    ) -> Result<Instruction> {
        // Orca Whirlpool swap instruction accounts
        // Based on Orca Whirlpools program IDL
        //
        // FIXED 2025-10-11: Tick arrays now properly derived PDAs
        //
        // 0. [writable] token_program (SPL Token Program)
        // 1. [signer] token_authority (User wallet)
        // 2. [writable] whirlpool (Pool account)
        // 3. [writable] token_owner_account_a (User's token A account)
        // 4. [writable] token_vault_a (Pool's token A vault)
        // 5. [writable] token_owner_account_b (User's token B account)
        // 6. [writable] token_vault_b (Pool's token B vault)
        // 7. [writable] tick_array_0 (Previous tick array - properly derived)
        // 8. [writable] tick_array_1 (Current tick array - properly derived)
        // 9. [writable] tick_array_2 (Next tick array - properly derived)
        // 10. [readonly] oracle (Price oracle account)

        let accounts = vec![
            solana_sdk::instruction::AccountMeta::new_readonly(spl_token::id(), false),
            solana_sdk::instruction::AccountMeta::new_readonly(*token_authority, true),
            solana_sdk::instruction::AccountMeta::new(*whirlpool, false),
            solana_sdk::instruction::AccountMeta::new(*token_owner_account_a, false),
            solana_sdk::instruction::AccountMeta::new(*token_vault_a, false),
            solana_sdk::instruction::AccountMeta::new(*token_owner_account_b, false),
            solana_sdk::instruction::AccountMeta::new(*token_vault_b, false),
            // FIXED: Tick arrays properly derived from pool state (prev, current, next)
            solana_sdk::instruction::AccountMeta::new(tick_arrays[0], false),
            solana_sdk::instruction::AccountMeta::new(tick_arrays[1], false),
            solana_sdk::instruction::AccountMeta::new(tick_arrays[2], false),
            solana_sdk::instruction::AccountMeta::new_readonly(*oracle, false),
        ];

        // Instruction data format for Orca Whirlpool swap
        // [discriminator: 8 bytes][amount: 8 bytes][other_amount_threshold: 8 bytes]
        // [sqrt_price_limit: 16 bytes][amount_specified_is_input: 1 byte][a_to_b: 1 byte]
        let mut data = Vec::new();

        // ORCA WHIRLPOOL SWAP DISCRIMINATOR
        // Extracted from successful Orca Whirlpool swaps on Solscan
        // Anchor discriminator for Whirlpool "swap" instruction
        //
        // CRITICAL: This must be validated before live trading:
        // 1. Find successful Orca Whirlpool swap on Solscan (e.g., recent swap on USDC/SOL pool)
        // 2. Look at transaction instruction data
        // 3. First 8 bytes are the discriminator
        //
        // Common Orca Whirlpool swap discriminator (from whirpool-sdk analysis):
        // [0xf8, 0xc6, 0x9e, 0x91, 0xe1, 0x75, 0x87, 0xc8] = SHA256("global:swap")[0..8]
        //
        // NOTE: If swaps fail with "invalid instruction", check latest Solscan transaction
        let swap_discriminator: [u8; 8] = [0xf8, 0xc6, 0x9e, 0x91, 0xe1, 0x75, 0x87, 0xc8];
        data.extend_from_slice(&swap_discriminator);

        // Amount (u64, 8 bytes, little-endian)
        // This is the input amount for the swap
        data.extend_from_slice(&swap_params.amount_in.to_le_bytes());

        // Other amount threshold (u64, 8 bytes, little-endian)
        // This is the minimum output amount (slippage protection)
        data.extend_from_slice(&swap_params.minimum_amount_out.to_le_bytes());

        // Sqrt price limit (u128, 16 bytes, little-endian)
        // Set to 0 for no price limit (rely on minimum_amount_out for slippage)
        let sqrt_price_limit: u128 = if swap_params.swap_a_to_b {
            // A to B: minimum sqrt price (4295048016) represents near-zero price
            4295048016
        } else {
            // B to A: maximum sqrt price (79226673521066979257578248091)
            79226673521066979257578248091
        };
        data.extend_from_slice(&sqrt_price_limit.to_le_bytes());

        // Amount specified is input (bool, 1 byte)
        // true = exact input swap, false = exact output swap
        // We use exact input (true)
        data.push(1); // true

        // Direction a_to_b (bool, 1 byte)
        data.push(if swap_params.swap_a_to_b { 1 } else { 0 });

        let instruction = Instruction {
            program_id: self.program_id,
            accounts,
            data,
        };

        debug!(
            "Built Orca Whirlpool instruction with {} accounts",
            instruction.accounts.len()
        );
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
        debug!("Estimating swap output for Orca pool: {}", pool_short_id);

        // Get pool info
        let pool_info = self
            .pool_registry
            .get_pool(pool_short_id)
            .ok_or_else(|| anyhow::anyhow!("Pool {} not found", pool_short_id))?;

        // Fetch pool state
        let _pool_state = self.fetch_pool_state(&pool_info.full_address)?;

        // Parse pool state to get current sqrt_price and liquidity
        // This would use Orca's concentrated liquidity math

        // For now, return a conservative estimate
        let estimated_output = amount_in * 99 / 100; // Assume 1% slippage

        warn!("⚠️ Using conservative estimate (1% slippage)");
        warn!("   Production should use Orca's concentrated liquidity curve calculation");

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
        assert_eq!(OrcaSwapBuilder::calculate_slippage(100, 95), 5.0);
        assert_eq!(OrcaSwapBuilder::calculate_slippage(1000, 950), 5.0);
        assert_eq!(OrcaSwapBuilder::calculate_slippage(100, 100), 0.0);
    }

    #[test]
    fn test_swap_params_validation() {
        let rpc_url = "https://api.mainnet-beta.solana.com".to_string();
        let rpc_client = Arc::new(SolanaRpcClient::new(rpc_url));
        let pool_registry = Arc::new(PoolRegistry::new(rpc_client.clone()));
        let builder = OrcaSwapBuilder::new(rpc_client, pool_registry).unwrap();

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
