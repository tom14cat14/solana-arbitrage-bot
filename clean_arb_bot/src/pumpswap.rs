// PumpSwap DEX swap implementation (FIXED 2025-10-14)
// Program ID: pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA
//
// PumpSwap is a constant-product AMM (like Uniswap v2 / Raydium v4)
// - Fee: 0.25% (0.2% to LPs, 0.05% to protocol)
// - Mechanism: x * y = k constant product
// - Tokens: Post-migration Pump.fun tokens
//
// FIXES APPLIED (2025-10-14):
// - Pool structure offsets corrected (+8 for Anchor discriminator)
// - Correct 12-account structure (Grok-verified from program analysis)
// - Vault PDAs derived with seeds ["vault", pool, mint]
// - Proper account ordering (user first, pool at position 7)
//
// Implementation based on Grok AI analysis of PumpSwap AMM program structure

use anyhow::{Context, Result};
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use std::str::FromStr;
use tracing::{debug, info};

use crate::rpc_client::SolanaRpcClient;

/// PumpSwap program ID
pub const PUMPSWAP_PROGRAM_ID: &str = "pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA";

/// SPL Token program ID
const SPL_TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

/// Associated Token Account program ID
const ASSOCIATED_TOKEN_PROGRAM_ID: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";

/// System program ID
const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";

/// BUY instruction discriminator (verified from on-chain tx 2025-10-13)
const BUY_DISCRIMINATOR: [u8; 8] = [102, 6, 61, 18, 1, 218, 235, 234];

/// SELL instruction discriminator (from ShredStream parser)
const SELL_DISCRIMINATOR: [u8; 8] = [51, 230, 133, 164, 1, 127, 131, 173];

/// PumpSwap pool accounts structure
/// Parsed from 300-byte pool account (PDA owned by PumpSwap program)
///
/// NOTE: pool_base_account and pool_quote_account are read from pool data
/// but NOT used in swap instructions - vaults are derived as PDAs instead!
#[derive(Debug, Clone)]
pub struct PumpSwapPool {
    pub pool_address: Pubkey,
    pub base_mint: Pubkey, // Token mint (offset 43, +8 for Anchor discriminator)
    pub quote_mint: Pubkey, // SOL/WSOL mint (offset 75, +8)
    pub pool_base_account: Pubkey, // Pool's token vault (offset 139, +8) - NOT USED IN SWAPS
    pub pool_quote_account: Pubkey, // Pool's SOL vault (offset 171, +8) - NOT USED IN SWAPS
}

/// PumpSwap swap builder
pub struct PumpSwapSwapBuilder {
    program_id: Pubkey,
    rpc_client: std::sync::Arc<SolanaRpcClient>,
}

impl PumpSwapSwapBuilder {
    /// Create new PumpSwap swap builder
    pub fn new(rpc_client: std::sync::Arc<SolanaRpcClient>) -> Result<Self> {
        let program_id =
            Pubkey::from_str(PUMPSWAP_PROGRAM_ID).context("Invalid PumpSwap program ID")?;

        info!("✅ PumpSwap swap builder initialized (FIXED 2025-10-13)");
        info!("   Program ID: {}", program_id);

        Ok(Self {
            program_id,
            rpc_client,
        })
    }

    /// Build a swap instruction for PumpSwap AMM
    ///
    /// CORRECT IMPLEMENTATION (2025-10-14): Uses Grok-verified 12-account structure
    /// with proper vault PDA derivation. This is the production-ready version.
    pub fn build_swap_instruction(
        &self,
        pool: &PumpSwapPool,
        user_wallet: &Pubkey,
        amount_in: u64,
        minimum_amount_out: u64,
        swap_a_to_b: bool, // true = SOL -> Token (BUY), false = Token -> SOL (SELL)
    ) -> Result<Instruction> {
        debug!("🔨 Building PumpSwap swap instruction (CORRECT 12-ACCOUNT STRUCTURE)");
        debug!("   Pool: {}", pool.pool_address);
        debug!("   Amount in: {}", amount_in);
        debug!("   Min amount out: {}", minimum_amount_out);
        debug!(
            "   Direction: {}",
            if swap_a_to_b {
                "BUY (SOL->Token)"
            } else {
                "SELL (Token->SOL)"
            }
        );

        // Get user's token accounts
        let user_base_account = spl_associated_token_account::get_associated_token_address(
            user_wallet,
            &pool.base_mint,
        );
        let user_quote_account = spl_associated_token_account::get_associated_token_address(
            user_wallet,
            &pool.quote_mint,
        );

        // Build instruction data
        let mut data = Vec::with_capacity(24);

        if swap_a_to_b {
            // BUY: SOL -> Token
            // Args: base_amount_out, max_quote_amount_in
            data.extend_from_slice(&BUY_DISCRIMINATOR);
            data.extend_from_slice(&minimum_amount_out.to_le_bytes()); // base_amount_out
            data.extend_from_slice(&amount_in.to_le_bytes()); // max_quote_amount_in
        } else {
            // SELL: Token -> SOL
            // Args: base_amount_in, min_quote_amount_out
            data.extend_from_slice(&SELL_DISCRIMINATOR);
            data.extend_from_slice(&amount_in.to_le_bytes()); // base_amount_in
            data.extend_from_slice(&minimum_amount_out.to_le_bytes()); // min_quote_amount_out
        }

        // Derive required PDAs (from Grok's PumpSwap AMM analysis)
        let (global_config, _) = Pubkey::find_program_address(&[b"global"], &self.program_id);
        let (event_authority, _) =
            Pubkey::find_program_address(&[b"__event_authority"], &self.program_id);

        // Determine mint order based on swap direction
        // For BUY (SOL→Token): mint_a = WSOL, mint_b = token
        // For SELL (Token→SOL): mint_a = token, mint_b = WSOL
        let (mint_a, mint_b, user_account_a, user_account_b) = if swap_a_to_b {
            // BUY: SOL → Token
            (
                pool.quote_mint,
                pool.base_mint,
                user_quote_account,
                user_base_account,
            )
        } else {
            // SELL: Token → SOL
            (
                pool.base_mint,
                pool.quote_mint,
                user_base_account,
                user_quote_account,
            )
        };

        // Derive vault PDAs with seeds: ["vault", pool, mint]
        let (vault_a, _) = Pubkey::find_program_address(
            &[b"vault", pool.pool_address.as_ref(), mint_a.as_ref()],
            &self.program_id,
        );
        let (vault_b, _) = Pubkey::find_program_address(
            &[b"vault", pool.pool_address.as_ref(), mint_b.as_ref()],
            &self.program_id,
        );

        debug!("📝 Derived PDAs:");
        debug!("   global_config: {}", global_config);
        debug!("   event_authority: {}", event_authority);
        debug!(
            "   vault_a ({}): {}",
            if swap_a_to_b { "WSOL" } else { "token" },
            vault_a
        );
        debug!(
            "   vault_b ({}): {}",
            if swap_a_to_b { "token" } else { "WSOL" },
            vault_b
        );

        // CORRECT 12-account structure from Grok analysis
        // CRITICAL: Order must match exactly or simulation will fail!
        let instruction = Instruction {
            program_id: self.program_id,
            accounts: vec![
                // 0: user (signer, writable)
                AccountMeta::new(*user_wallet, true),
                // 1: user_token_account_a (writable) - input token account
                AccountMeta::new(user_account_a, false),
                // 2: user_token_account_b (writable) - output token account
                AccountMeta::new(user_account_b, false),
                // 3: vault_a (writable) - pool vault for token A (PDA)
                AccountMeta::new(vault_a, false),
                // 4: vault_b (writable) - pool vault for token B (PDA)
                AccountMeta::new(vault_b, false),
                // 5: mint_a (read-only) - input token mint
                AccountMeta::new_readonly(mint_a, false),
                // 6: mint_b (read-only) - output token mint
                AccountMeta::new_readonly(mint_b, false),
                // 7: pool (read-only) - AMM pool state account
                AccountMeta::new_readonly(pool.pool_address, false),
                // 8: global_config (read-only, PDA)
                AccountMeta::new_readonly(global_config, false),
                // 9: event_authority (read-only, PDA)
                AccountMeta::new_readonly(event_authority, false),
                // 10: token_program (read-only)
                AccountMeta::new_readonly(Pubkey::from_str(SPL_TOKEN_PROGRAM_ID).unwrap(), false),
                // 11: associated_token_program (read-only)
                AccountMeta::new_readonly(
                    Pubkey::from_str(ASSOCIATED_TOKEN_PROGRAM_ID).unwrap(),
                    false,
                ),
            ],
            data,
        };

        debug!(
            "✅ PumpSwap swap instruction built ({} accounts, Grok-verified)",
            instruction.accounts.len()
        );
        Ok(instruction)
    }

    /// Fetch pool state from on-chain data
    /// Pool structure (300 bytes, FIXED 2025-10-14 - includes Anchor 8-byte discriminator):
    /// Offset 0-8:   Anchor discriminator (8 bytes)
    /// Offset 8-9:   pool_bump (1), index (2), creator (32)
    /// Offset 43:  base_mint (32) ← TOKEN MINT (was 35, +8 for discriminator)
    /// Offset 75:  quote_mint (32) ← SOL/WSOL MINT (was 67, +8)
    /// Offset 107: lp_mint (32)
    /// Offset 139: pool_base_token_account (32) ← TOKEN VAULT (was 131, +8)
    /// Offset 171: pool_quote_token_account (32) ← SOL VAULT (was 163, +8)
    /// Offset 203: lp_supply (8)
    pub fn fetch_pool_info(&self, pool_address: &Pubkey) -> Result<PumpSwapPool> {
        debug!("🔍 Fetching PumpSwap pool info for: {}", pool_address);

        // Fetch pool account data
        let pool_data = self
            .rpc_client
            .get_account_data(pool_address)
            .context("Failed to fetch PumpSwap pool data")?;

        if pool_data.len() < 203 {
            return Err(anyhow::anyhow!(
                "Invalid PumpSwap pool data length: {} (expected >= 203)",
                pool_data.len()
            ));
        }

        // Parse using CORRECT offsets (including Anchor 8-byte discriminator)
        let base_mint = Pubkey::try_from(&pool_data[43..75])
            .map_err(|_| anyhow::anyhow!("Invalid base mint pubkey"))?;
        let quote_mint = Pubkey::try_from(&pool_data[75..107])
            .map_err(|_| anyhow::anyhow!("Invalid quote mint pubkey"))?;
        let pool_base_account = Pubkey::try_from(&pool_data[139..171])
            .map_err(|_| anyhow::anyhow!("Invalid pool base account pubkey"))?;
        let pool_quote_account = Pubkey::try_from(&pool_data[171..203])
            .map_err(|_| anyhow::anyhow!("Invalid pool quote account pubkey"))?;

        debug!("✅ PumpSwap pool info parsed");
        debug!("   Base mint (token): {}", base_mint);
        debug!("   Quote mint (SOL): {}", quote_mint);
        debug!("   Pool base vault: {}", pool_base_account);
        debug!("   Pool quote vault: {}", pool_quote_account);

        Ok(PumpSwapPool {
            pool_address: *pool_address,
            base_mint,
            quote_mint,
            pool_base_account,
            pool_quote_account,
        })
    }

    /// Validate that pool is PumpSwap AMM
    pub fn is_pumpswap_pool(&self, pool_address: &Pubkey) -> Result<bool> {
        match self.rpc_client.get_account_owner(pool_address) {
            Ok(owner) => {
                let pumpswap_program = Pubkey::from_str(PUMPSWAP_PROGRAM_ID).unwrap();
                Ok(owner == pumpswap_program)
            }
            Err(e) => Err(anyhow::anyhow!("Failed to verify pool owner: {}", e)),
        }
    }
}
