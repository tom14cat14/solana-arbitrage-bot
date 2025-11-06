// High-level swap executor coordinator
//
// Ties together all DEX swap components:
// - RPC client for blockchain interaction
// - Pool registry for address resolution
// - Meteora swap builder for instruction generation
// - JITO bundle client for atomic execution
//
// Provides simple API for arbitrage engine

use anyhow::{Context, Result};
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction,
    hash::Hash,
    instruction::Instruction,
    pubkey::Pubkey,
    signature::Signature,
    signer::Signer,
    transaction::Transaction,
};
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::{
    types::{DexType, SwapParams},
    rpc_client::SolanaRpcClient,
    pool_registry::PoolRegistry,
    meteora::MeteoraSwapBuilder,
    pumpswap::PumpSwapSwapBuilder,
    orca::OrcaSwapBuilder,
    raydium::RaydiumSwapBuilder,
    humidifi::HumidiFiSwapBuilder,
};
use crate::jito_bundle_client::JitoBundleClient;

/// High-level swap executor that coordinates all swap operations
pub struct SwapExecutor {
    /// RPC client for blockchain operations
    rpc_client: Arc<SolanaRpcClient>,
    /// Pool registry for address lookups
    pool_registry: Arc<PoolRegistry>,
    /// Meteora swap builder
    meteora_builder: MeteoraSwapBuilder,
    /// Orca swap builder
    orca_builder: OrcaSwapBuilder,
    /// PumpSwap swap builder
    pumpswap_builder: PumpSwapSwapBuilder,
    /// Raydium swap builder
    raydium_builder: RaydiumSwapBuilder,
    /// HumidiFi swap builder
    humidifi_builder: Option<HumidiFiSwapBuilder>,
    /// JITO bundle client for atomic execution (optional)
    jito_client: Option<Arc<JitoBundleClient>>,
    /// Default compute budget (micro-lamports per compute unit)
    compute_unit_price: u64,
    /// Default compute unit limit
    compute_unit_limit: u32,
}

impl SwapExecutor {
    /// Create new swap executor
    pub fn new(
        rpc_client: Arc<SolanaRpcClient>,
        pool_registry: Arc<PoolRegistry>,
        jito_client: Option<Arc<JitoBundleClient>>,
    ) -> Result<Self> {
        // Initialize Meteora builder
        let meteora_builder = MeteoraSwapBuilder::new(
            rpc_client.clone(),
            pool_registry.clone(),
        )?;

        // Initialize Orca builder
        let orca_builder = OrcaSwapBuilder::new(
            rpc_client.clone(),
            pool_registry.clone(),
        )?;

        // Initialize PumpSwap builder
        let pumpswap_builder = PumpSwapSwapBuilder::new(rpc_client.clone())?;

        // Initialize Raydium builder
        let raydium_builder = RaydiumSwapBuilder::new(
            rpc_client.clone(),
            pool_registry.clone(),
        )?;

        // Initialize HumidiFi builder (may fail if program ID is incorrect)
        let humidifi_builder = match HumidiFiSwapBuilder::new() {
            Ok(builder) => {
                info!("✅ HumidiFi swap builder initialized");
                Some(builder)
            }
            Err(e) => {
                warn!("⚠️ HumidiFi swap builder failed to initialize: {}", e);
                None
            }
        };

        info!("✅ Swap executor initialized");
        info!("   DEX support: Meteora DLMM/DAMM V2, Orca Whirlpools, Raydium CPMM, PumpSwap{}",
            if humidifi_builder.is_some() { ", HumidiFi" } else { "" });
        info!("   JITO bundles: {}", if jito_client.is_some() { "enabled" } else { "disabled" });

        Ok(Self {
            rpc_client,
            pool_registry,
            meteora_builder,
            orca_builder,
            pumpswap_builder,
            raydium_builder,
            humidifi_builder,
            jito_client,
            compute_unit_price: 1000,     // 1000 micro-lamports (0.001 lamports per CU)
            compute_unit_limit: 200_000,  // 200k compute units
        })
    }

    /// CYCLE-5 FIX: Check if RPC circuit breaker is tripped
    /// Returns error if too many consecutive RPC failures have occurred
    pub fn check_circuit_breaker(&self) -> Result<()> {
        self.rpc_client.check_circuit_breaker()
    }

    /// Execute a single swap on a DEX
    ///
    /// CYCLE-7: MANDATORY SIMULATION (Grok recommendation for bulletproof trading)
    /// ALL transactions are simulated before execution - no exceptions
    ///
    /// # Arguments
    /// * `dex_type` - Type of DEX (Meteora, Orca, etc.)
    /// * `pool_short_id` - 8-char pool ID from ShredStream
    /// * `swap_params` - Swap parameters
    /// * `wallet` - User's wallet (must be a Signer)
    ///
    /// # Returns
    /// Transaction signature if successful
    pub async fn execute_swap<T: Signer>(
        &self,
        dex_type: &DexType,
        pool_short_id: &str,
        swap_params: &SwapParams,
        wallet: &T,
    ) -> Result<Signature> {
        info!("🔄 Executing swap on {:?}", dex_type);
        info!("   Pool: {}", pool_short_id);
        info!("   Amount in: {}", swap_params.amount_in);
        info!("   Min out: {}", swap_params.minimum_amount_out);

        // HIGH-2 FIX: Validate slippage tolerance
        // Ensure minimum_amount_out is reasonable (not allowing >5% slippage)
        if let Some(expected_out) = swap_params.expected_amount_out {
            if swap_params.minimum_amount_out > 0 {
                let slippage = ((expected_out - swap_params.minimum_amount_out) as f64 / expected_out as f64) * 100.0;
                if slippage > 5.0 {
                    return Err(anyhow::anyhow!(
                        "Slippage validation failed: {:.2}% exceeds maximum 5%\n   Expected: {}, Min: {}",
                        slippage, expected_out, swap_params.minimum_amount_out
                    ));
                }
                if slippage < 0.0 {
                    return Err(anyhow::anyhow!(
                        "Invalid slippage: minimum_amount_out ({}) exceeds expected_amount_out ({})",
                        swap_params.minimum_amount_out, expected_out
                    ));
                }
                debug!("✅ Slippage validation passed: {:.2}%", slippage);
            }
        }

        // Build swap instruction based on DEX type (now async for pool resolution)
        let swap_ix = self.build_swap_instruction(
            dex_type,
            pool_short_id,
            swap_params,
            &wallet.pubkey(),
        ).await?;

        // Get recent blockhash
        let recent_blockhash = self.rpc_client
            .get_latest_blockhash()
            .context("Failed to get recent blockhash")?;

        // Build complete transaction with compute budget
        let transaction = self.build_transaction(
            vec![swap_ix],
            wallet,
            recent_blockhash,
        )?;

        // CYCLE-7: MANDATORY SIMULATION (Grok recommendation)
        // Catches failed swaps without cost - bulletproof safety
        info!("🧪 Simulating transaction before execution...");
        let sim_result = self.rpc_client.simulate_transaction(&transaction)?;

        if !sim_result {
            return Err(anyhow::anyhow!(
                "Transaction simulation failed - trade would revert on-chain. Rejected to protect capital."
            ));
        }

        info!("✅ Simulation passed - executing real transaction");


        // Send transaction
        let signature = self.rpc_client
            .send_transaction(&transaction)
            .context("Failed to send transaction")?;

        info!("📤 Swap transaction sent: {}", signature);

        // CRITICAL: Wait for confirmation with timeout (5 seconds)
        // Solana-optimized: Transactions typically confirm in 1-2 seconds (400ms slot time)
        // Never assume transaction succeeded until confirmed on-chain
        match tokio::time::timeout(
            tokio::time::Duration::from_secs(5),
            self.confirm_transaction(&signature)
        ).await {
            Ok(Ok(confirmed)) => {
                if confirmed {
                    info!("✅ Swap transaction confirmed: {}", signature);
                    Ok(signature)
                } else {
                    Err(anyhow::anyhow!(
                        "Transaction failed to confirm within timeout: {}",
                        signature
                    ))
                }
            }
            Ok(Err(e)) => {
                Err(anyhow::anyhow!(
                    "Error while confirming transaction {}: {}",
                    signature, e
                ))
            }
            Err(_) => {
                Err(anyhow::anyhow!(
                    "Transaction confirmation timeout (5s) for: {}",
                    signature
                ))
            }
        }
    }

    /// Execute a triangle arbitrage (3 swaps atomically)
    ///
    /// # Arguments
    /// * `leg1` - First swap (e.g., SOL → TokenA)
    /// * `leg2` - Second swap (e.g., TokenA → TokenB)
    /// * `leg3` - Third swap (e.g., TokenB → SOL)
    /// * `wallet` - User's wallet
    /// * `use_jito` - If true, submit via JITO bundle for MEV protection
    ///
    /// # Returns
    /// Transaction signature or bundle ID
    pub async fn execute_triangle<T: Signer>(
        &self,
        leg1: (&DexType, &str, &SwapParams),
        leg2: (&DexType, &str, &SwapParams),
        leg3: (&DexType, &str, &SwapParams),
        wallet: &T,
        use_jito: bool,
    ) -> Result<String> {
        info!("🔺 Executing triangle arbitrage");
        info!("   Leg 1: {:?} pool {}", leg1.0, leg1.1);
        info!("   Leg 2: {:?} pool {}", leg2.0, leg2.1);
        info!("   Leg 3: {:?} pool {}", leg3.0, leg3.1);

        let user_pubkey = wallet.pubkey();

        // Build all three swap instructions (async for pool resolution)
        let ix1 = self.build_swap_instruction(leg1.0, leg1.1, leg1.2, &user_pubkey).await?;
        let ix2 = self.build_swap_instruction(leg2.0, leg2.1, leg2.2, &user_pubkey).await?;
        let ix3 = self.build_swap_instruction(leg3.0, leg3.1, leg3.2, &user_pubkey).await?;

        debug!("✅ Built all 3 swap instructions");

        // Get recent blockhash
        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;

        // Build transaction with all swaps
        let transaction = self.build_transaction(
            vec![ix1, ix2, ix3],
            wallet,
            recent_blockhash,
        )?;

        // Simulate first
        info!("🧪 Simulating triangle transaction...");
        let sim_result = self.rpc_client.simulate_transaction(&transaction)?;

        if !sim_result {
            return Err(anyhow::anyhow!(
                "Triangle arbitrage simulation failed - would revert on-chain. \
                Likely slippage or insufficient liquidity."
            ));
        }

        info!("✅ Triangle simulation passed");

        // Execute via JITO bundle or regular transaction
        if use_jito && self.jito_client.is_some() {
            info!("💎 Submitting via JITO bundle for MEV protection...");

            // TODO: Use JITO client to submit bundle
            // let bundle_id = self.jito_client.as_ref().unwrap()
            //     .submit_bundle(&transaction)
            //     .await?;

            warn!("⚠️ JITO bundle submission not yet wired up");
            warn!("   Falling back to regular transaction");

            let signature = self.rpc_client.send_transaction(&transaction)?;
            info!("✅ Triangle transaction sent: {}", signature);

            Ok(signature.to_string())
        } else {
            // Regular transaction
            let signature = self.rpc_client.send_transaction(&transaction)?;
            info!("✅ Triangle transaction sent: {}", signature);

            Ok(signature.to_string())
        }
    }

    /// Build triangle transaction without submitting (for queue-based JITO submission)
    pub async fn build_triangle_transaction<T: Signer>(
        &self,
        leg1: (&DexType, &str, &SwapParams),
        leg2: (&DexType, &str, &SwapParams),
        leg3: (&DexType, &str, &SwapParams),
        wallet: &T,
    ) -> Result<Transaction> {
        let user_pubkey = wallet.pubkey();

        // Build all three swap instructions (async for pool resolution)
        let ix1 = self.build_swap_instruction(leg1.0, leg1.1, leg1.2, &user_pubkey).await?;
        let ix2 = self.build_swap_instruction(leg2.0, leg2.1, leg2.2, &user_pubkey).await?;
        let ix3 = self.build_swap_instruction(leg3.0, leg3.1, leg3.2, &user_pubkey).await?;

        // Get recent blockhash
        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;

        // Build and return transaction
        let transaction = self.build_transaction(
            vec![ix1, ix2, ix3],
            wallet,
            recent_blockhash,
        )?;

        Ok(transaction)
    }

    /// Build triangle transaction with JITO tip INCLUDED (SECURE METHOD)
    ///
    /// **CRITICAL SECURITY FIX**: This method includes the JITO tip INSIDE the same
    /// transaction as the swap instructions, preventing uncle block unbundling.
    ///
    /// Per Jito docs: "Always make sure your Jito tip transaction is in the same
    /// transaction that is running the MEV strategy"
    ///
    /// # Arguments
    /// * `leg1` - First swap parameters
    /// * `leg2` - Second swap parameters
    /// * `leg3` - Third swap parameters
    /// * `wallet` - User's wallet (signer)
    /// * `tip_lamports` - Tip amount (minimum 1000 lamports)
    /// * `tip_account` - Jito tip account pubkey
    ///
    /// # Returns
    /// Complete signed transaction ready for JITO bundle submission
    pub async fn build_triangle_with_tip<T: Signer>(
        &self,
        leg1: (&DexType, &str, &SwapParams),
        leg2: (&DexType, &str, &SwapParams),
        leg3: (&DexType, &str, &SwapParams),
        wallet: &T,
        tip_lamports: u64,
        tip_account: &Pubkey,
    ) -> Result<Transaction> {
        let user_pubkey = wallet.pubkey();

        // Build all three swap instructions (async for pool resolution)
        let ix1 = self.build_swap_instruction(leg1.0, leg1.1, leg1.2, &user_pubkey).await?;
        let ix2 = self.build_swap_instruction(leg2.0, leg2.1, leg2.2, &user_pubkey).await?;
        let ix3 = self.build_swap_instruction(leg3.0, leg3.1, leg3.2, &user_pubkey).await?;

        info!("✅ Built all 3 swap instructions");

        // Build JITO tip instruction
        let tip_ix = solana_sdk::system_instruction::transfer(
            &user_pubkey,
            tip_account,
            tip_lamports,
        );

        info!("✅ Built JITO tip instruction: {} lamports (0.{:06} SOL) to {}",
              tip_lamports, tip_lamports / 1000, tip_account);

        // SECURITY FIX (2025-10-08): Combine swap instructions + tip
        // Note: build_transaction() will add compute budget instructions automatically
        let all_instructions = vec![ix1, ix2, ix3, tip_ix];

        info!("🔒 SECURE: Tip included IN swap transaction (prevents unbundling)");

        // Get recent blockhash
        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;

        // Build transaction with all instructions atomically
        let transaction = self.build_transaction(
            all_instructions,
            wallet,
            recent_blockhash,
        )?;

        info!("✅ Built SECURE transaction: 3 swaps + 1 tip = {} total instructions",
              transaction.message.instructions.len());

        Ok(transaction)
    }

    /// Build triangle transaction with PROFIT-BASED JITO tip (RECOMMENDED)
    ///
    /// This method automatically calculates optimal tip based on expected profit:
    /// - Minimum: 100,000 lamports (0.0001 SOL) - 95th percentile
    /// - Base: 10% of expected profit
    /// - Maximum: 20% of expected profit
    ///
    /// # Arguments
    /// * `leg1` - First swap parameters
    /// * `leg2` - Second swap parameters
    /// * `leg3` - Third swap parameters
    /// * `wallet` - User's wallet (signer)
    /// * `expected_profit_lamports` - Expected profit from arbitrage
    /// * `tip_account` - Jito tip account pubkey
    ///
    /// # Returns
    /// Complete signed transaction ready for JITO bundle submission
    ///
    /// # Example
    /// ```ignore
    /// // Arbitrage with 0.5 SOL expected profit
    /// let tx = swap_executor.build_triangle_with_profit_based_tip(
    ///     leg1, leg2, leg3,
    ///     &wallet,
    ///     500_000_000, // 0.5 SOL expected profit
    ///     &tip_account,
    /// ).await?;
    /// // Tip will be: 50,000,000 lamports (10% of 0.5 SOL = 0.05 SOL)
    /// ```
    pub async fn build_triangle_with_profit_based_tip<T: Signer>(
        &self,
        leg1: (&DexType, &str, &SwapParams),
        leg2: (&DexType, &str, &SwapParams),
        leg3: (&DexType, &str, &SwapParams),
        wallet: &T,
        expected_profit_lamports: u64,
        tip_account: &Pubkey,
    ) -> Result<Transaction> {
        // Calculate optimal tip based on profit (requires JITO client)
        let tip_lamports = if let Some(jito_client) = &self.jito_client {
            jito_client.calculate_optimal_tip_with_profit(Some(expected_profit_lamports))
        } else {
            // Fallback if no JITO client: 10% of profit, min 100k lamports
            let tip = (expected_profit_lamports as f64 * 0.10) as u64;
            tip.max(100_000)
        };

        info!("💰 Profit-based tip calculation:");
        info!("   Expected profit: {} lamports (0.{:06} SOL)",
              expected_profit_lamports, expected_profit_lamports / 1000);
        info!("   Calculated tip: {} lamports (0.{:06} SOL)",
              tip_lamports, tip_lamports / 1000);
        info!("   Tip percentage: {:.1}%",
              (tip_lamports as f64 / expected_profit_lamports as f64) * 100.0);

        // Build transaction with calculated tip
        self.build_triangle_with_tip(
            leg1, leg2, leg3,
            wallet,
            tip_lamports,
            tip_account,
        ).await
    }

    /// Build swap instruction for given DEX type (async for pool resolution)
    async fn build_swap_instruction(
        &self,
        dex_type: &DexType,
        pool_short_id: &str,
        swap_params: &SwapParams,
        user_pubkey: &Pubkey,
    ) -> Result<Instruction> {
        match dex_type {
            // Meteora variants (all use same builder)
            DexType::MeteoraDammV1 | DexType::MeteoraDammV2 | DexType::MeteoraDlmm => {
                self.meteora_builder.build_swap_instruction(
                    pool_short_id,
                    swap_params,
                    user_pubkey,
                ).await
            }

            // Orca variants
            DexType::OrcaWhirlpools | DexType::OrcaLegacy => {
                // Both use same Orca builder (handles both variants)
                self.orca_builder.build_swap_instruction(
                    pool_short_id,
                    swap_params,
                    user_pubkey,
                ).await
            }

            // Raydium variants (all use same builder)
            DexType::RaydiumAmmV4 | DexType::RaydiumClmm | DexType::RaydiumCpmm | DexType::RaydiumStable => {
                self.raydium_builder.build_swap_instruction(
                    pool_short_id,
                    swap_params,
                    user_pubkey,
                ).await
            }

            DexType::PumpSwap => {
                // Resolve pool address from short ID
                let pool_address = self.pool_registry
                    .resolve_pool_address(pool_short_id, dex_type)
                    .await
                    .context(format!("Failed to resolve PumpSwap pool address for {}", pool_short_id))?;

                // Fetch pool info from on-chain data
                let pool_info = self.pumpswap_builder.fetch_pool_info(&pool_address)
                    .context("Failed to fetch PumpSwap pool info")?;

                // Build swap instruction
                self.pumpswap_builder.build_swap_instruction(
                    &pool_info,
                    user_pubkey,
                    swap_params.amount_in,
                    swap_params.minimum_amount_out,
                    swap_params.swap_a_to_b,
                )
            }

            // HumidiFi dark pool
            DexType::HumidiFi => {
                debug!("🐸 Building HumidiFi swap instruction for pool {}", pool_short_id);

                // Get HumidiFi builder (should be initialized)
                let builder = self.humidifi_builder.as_ref()
                    .ok_or_else(|| anyhow::anyhow!("HumidiFi builder not initialized"))?;

                // Resolve pool address from short ID
                let pool_address = self.pool_registry
                    .resolve_pool_address(pool_short_id, dex_type)
                    .await
                    .context(format!("Failed to resolve HumidiFi pool address for {}", pool_short_id))?;

                // TODO: Fetch actual token mints from pool account data (like Meteora/Orca do)
                // For now, use common token mints (SOL/USDC) since HumidiFi primarily deals with these pairs
                // This is a temporary solution that works for most HumidiFi pools
                use solana_sdk::pubkey;
                let sol_mint = pubkey!("So11111111111111111111111111111111111111112"); // Wrapped SOL
                let usdc_mint = pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"); // USDC

                // Determine token direction based on swap_a_to_b
                let (token_a, token_b) = if swap_params.swap_a_to_b {
                    (sol_mint, usdc_mint)
                } else {
                    (usdc_mint, sol_mint)
                };

                // Build swap instruction using legacy method (raw addresses)
                let instructions = builder.build_swap_instruction_legacy(
                    pool_address,
                    *user_pubkey,
                    token_a,
                    token_b,
                    swap_params.amount_in,
                    swap_params.minimum_amount_out,
                    swap_params.swap_a_to_b,
                ).await?;

                // Return first instruction (should be single swap instruction)
                instructions.into_iter().next()
                    .ok_or_else(|| anyhow::anyhow!("HumidiFi builder returned no instructions"))
            }

            // Not yet implemented DEXes - gracefully skip
            DexType::Jupiter | DexType::Serum | DexType::Aldrin | DexType::Saros |
            DexType::Crema | DexType::Cropper | DexType::Lifinity | DexType::Fluxbeam => {
                warn!("⚠️ DEX {:?} not yet implemented - skipping opportunity on pool {}", dex_type, pool_short_id);
                warn!("   To enable this DEX, implement builder in src/{}.rs",
                      format!("{:?}", dex_type).to_lowercase());
                Err(anyhow::anyhow!("DEX {:?} implementation pending", dex_type))
            }
        }
    }

    /// Build complete transaction with compute budget instructions
    fn build_transaction<T: Signer>(
        &self,
        swap_instructions: Vec<Instruction>,
        wallet: &T,
        recent_blockhash: Hash,
    ) -> Result<Transaction> {
        let mut instructions = Vec::new();

        // HIGH FIX: Dynamic compute budget based on swap complexity
        let estimated_cu = match swap_instructions.len() {
            1 => 100_000,   // Single swap
            2 => 200_000,   // 2-leg arbitrage
            3 => 300_000,   // Triangle arbitrage
            _ => 400_000,   // Complex multi-hop
        };

        // Add 20% safety buffer
        let compute_limit = (estimated_cu as f64 * 1.2) as u32;

        debug!("Estimated compute units: {} (with 20% buffer: {})", estimated_cu, compute_limit);

        // Add compute budget instructions first
        instructions.push(
            ComputeBudgetInstruction::set_compute_unit_price(self.compute_unit_price)
        );
        instructions.push(
            ComputeBudgetInstruction::set_compute_unit_limit(compute_limit)
        );

        // Add swap instructions
        instructions.extend(swap_instructions);

        // Create transaction
        let mut transaction = Transaction::new_with_payer(
            &instructions,
            Some(&wallet.pubkey()),
        );

        // Sign transaction
        transaction.sign(&[wallet], recent_blockhash);

        debug!("✅ Built transaction with {} instructions", instructions.len());

        Ok(transaction)
    }

    /// Set compute unit price (micro-lamports per compute unit)
    pub fn set_compute_unit_price(&mut self, price: u64) {
        self.compute_unit_price = price;
        debug!("Set compute unit price: {} micro-lamports", price);
    }

    /// Set compute unit limit
    pub fn set_compute_unit_limit(&mut self, limit: u32) {
        self.compute_unit_limit = limit;
        debug!("Set compute unit limit: {} CUs", limit);
    }

    /// Estimate swap output (for slippage calculation)
    pub fn estimate_swap_output(
        &self,
        dex_type: &DexType,
        pool_short_id: &str,
        amount_in: u64,
        swap_a_to_b: bool,
    ) -> Result<u64> {
        match dex_type {
            // Meteora variants (all use same builder)
            DexType::MeteoraDammV1 | DexType::MeteoraDammV2 | DexType::MeteoraDlmm => {
                self.meteora_builder.estimate_swap_output(
                    pool_short_id,
                    amount_in,
                    swap_a_to_b,
                )
            }

            // Orca variants
            DexType::OrcaWhirlpools | DexType::OrcaLegacy => {
                // Conservative estimate for Orca (1% slippage)
                warn!("⚠️ Orca output estimation not yet implemented - using 1% slippage estimate");
                Ok(amount_in * 99 / 100)
            }

            // Raydium variants (all use same builder)
            DexType::RaydiumAmmV4 | DexType::RaydiumClmm | DexType::RaydiumCpmm | DexType::RaydiumStable => {
                self.raydium_builder.estimate_swap_output(
                    pool_short_id,
                    amount_in,
                    swap_a_to_b,
                )
            }

            DexType::PumpSwap => {
                // Conservative estimate for PumpSwap (1% slippage)
                warn!("⚠️ PumpSwap output estimation not yet implemented - using 1% slippage estimate");
                Ok(amount_in * 99 / 100)
            }

            DexType::HumidiFi => {
                // Conservative estimate for HumidiFi dark pool (0.5% slippage - highly efficient)
                warn!("⚠️ HumidiFi output estimation not yet implemented - using 0.5% slippage estimate (dark pool efficiency)");
                Ok(amount_in * 995 / 1000)  // HumidiFi is known for very low slippage
            }

            // Not yet implemented DEXes - conservative estimate
            DexType::Jupiter | DexType::Serum | DexType::Aldrin | DexType::Saros |
            DexType::Crema | DexType::Cropper | DexType::Lifinity | DexType::Fluxbeam => {
                warn!("⚠️ DEX {:?} output estimation not implemented - using 1% slippage estimate", dex_type);
                Ok(amount_in * 99 / 100)
            }
        }
    }

    /// Calculate recommended minimum output with slippage tolerance
    ///
    /// # Arguments
    /// * `expected_output` - Expected output amount
    /// * `slippage_bps` - Slippage tolerance in basis points (100 = 1%)
    ///
    /// # Returns
    /// Minimum output amount accounting for slippage
    pub fn calculate_min_output_with_slippage(
        expected_output: u64,
        slippage_bps: u64,
    ) -> u64 {
        // Calculate: expected * (10000 - slippage_bps) / 10000
        expected_output
            .saturating_mul(10000 - slippage_bps)
            .saturating_div(10000)
    }

    /// Confirm transaction on-chain
    async fn confirm_transaction(&self, signature: &Signature) -> Result<bool> {
        // Poll for confirmation status
        for _ in 0..30 {
            match self.rpc_client.get_transaction_status(signature) {
                Ok(Some(status)) => {
                    return Ok(status);
                }
                Ok(None) => {
                    // Not yet confirmed, wait and retry
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                }
                Err(e) => {
                    warn!("Error checking transaction status: {}", e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                }
            }
        }
        Ok(false) // Not confirmed after 30 attempts
    }

    /// Health check - verify all components are working
    pub fn health_check(&self) -> Result<bool> {
        debug!("Running swap executor health check...");

        // Check RPC connection
        if !self.rpc_client.health_check()? {
            warn!("❌ RPC health check failed");
            return Ok(false);
        }

        // Check pool registry has pools
        let pool_count = self.pool_registry.pool_count();
        if pool_count == 0 {
            warn!("⚠️ Pool registry is empty - run populate_*_pools() first");
        } else {
            debug!("✅ Pool registry has {} pools", pool_count);
        }

        // Check JITO client if enabled
        if let Some(ref _jito) = self.jito_client {
            debug!("✅ JITO client enabled");
        }

        info!("✅ Swap executor health check passed");
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_min_output_calculation() {
        // 1% slippage (100 bps)
        assert_eq!(
            SwapExecutor::calculate_min_output_with_slippage(1000, 100),
            990
        );

        // 0.5% slippage (50 bps)
        assert_eq!(
            SwapExecutor::calculate_min_output_with_slippage(1000, 50),
            995
        );

        // 5% slippage (500 bps)
        assert_eq!(
            SwapExecutor::calculate_min_output_with_slippage(1000, 500),
            950
        );

        // 0% slippage
        assert_eq!(
            SwapExecutor::calculate_min_output_with_slippage(1000, 0),
            1000
        );
    }

    #[test]
    fn test_swap_executor_creation() {
        let rpc_url = "https://api.mainnet-beta.solana.com".to_string();
        let rpc_client = Arc::new(SolanaRpcClient::new(rpc_url));
        let pool_registry = Arc::new(PoolRegistry::new(rpc_client.clone()));

        let executor = SwapExecutor::new(
            rpc_client,
            pool_registry,
            None,
        ).unwrap();

        assert_eq!(executor.compute_unit_price, 1000);
        assert_eq!(executor.compute_unit_limit, 200_000);
    }
}
