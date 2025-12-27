//! Meteora DAMM V2 (LB-CLMM) Swap Implementation - Client Side
//! Complete implementation using manual instruction building for client-side execution

use std::str::FromStr;
use std::sync::Arc;
use anyhow::{anyhow, Result};
use borsh::{BorshSerialize, BorshDeserialize};
use crate::SolanaRpcClient;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use spl_associated_token_account::get_associated_token_address;
use spl_token::id as token_program_id;
use tracing::{info, warn};

// Meteora LB-CLMM program ID (mainnet)
pub const LB_CLMM_PROGRAM_ID: &str = "LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo";

// Event authority for Meteora (standard PDA)
pub const EVENT_AUTHORITY: &str = "6XzaKuAwqP7Nn37vwRdUqpuzNXknkBqjWq3c3h8qQXhE";

/// Swap instruction data for Meteora LB-CLMM
#[derive(BorshSerialize, BorshDeserialize)]
struct SwapInstructionData {
    /// Amount to swap in
    amount_in: u64,
    /// Minimum amount out (with slippage)
    min_amount_out: u64,
}

/// Fetch token mint addresses from pool
async fn get_pool_token_mints(
    rpc_client: &SolanaRpcClient,
    pool_address: &Pubkey,
) -> Result<(Pubkey, Pubkey)> {
    // Fetch pool account data
    let account_data = rpc_client.get_account_data(pool_address)
        .map_err(|e| anyhow!("Failed to fetch pool account: {}", e))?;

    // For Meteora LB pair, token mints are at specific offsets in account data
    // This is a simplified version - in production, deserialize the full LbPair struct
    if account_data.len() < 128 {
        return Err(anyhow!("Invalid pool account data"));
    }

    // Token X mint is at offset 8 (after discriminator)
    let token_x_bytes = &account_data[8..40];
    let token_x_mint = Pubkey::try_from(token_x_bytes)
        .map_err(|_| anyhow!("Invalid token X mint bytes"))?;

    // Token Y mint is at offset 40
    let token_y_bytes = &account_data[40..72];
    let token_y_mint = Pubkey::try_from(token_y_bytes)
        .map_err(|_| anyhow!("Invalid token Y mint bytes"))?;

    Ok((token_x_mint, token_y_mint))
}

/// Build Meteora swap instruction manually
pub async fn build_meteora_swap_instruction(
    rpc_client: Arc<SolanaRpcClient>,
    pool_address: &str,
    position_size_lamports: u64,
    user_wallet: &Pubkey,
    slippage_tolerance: f64,
    swap_for_y: bool,  // true = X to Y, false = Y to X
) -> Result<Instruction> {
    info!("🏗️ Building Meteora swap instruction:");
    info!("   Pool: {}", pool_address);
    info!("   Amount: {} lamports", position_size_lamports);
    info!("   Slippage: {}%", slippage_tolerance * 100.0);

    let lb_pair = Pubkey::from_str(pool_address)?;
    let program_id = Pubkey::from_str(LB_CLMM_PROGRAM_ID)?;

    // Get token mints from pool
    let (token_x_mint, token_y_mint) = get_pool_token_mints(&rpc_client, &lb_pair).await?;

    info!("📍 Pool tokens:");
    info!("   Token X: {}", token_x_mint);
    info!("   Token Y: {}", token_y_mint);

    // Get user's token accounts (ATAs)
    let user_token_x = get_associated_token_address(user_wallet, &token_x_mint);
    let user_token_y = get_associated_token_address(user_wallet, &token_y_mint);

    // Calculate minimum amount out with slippage protection
    let min_amount_out = (position_size_lamports as f64 * (1.0 - slippage_tolerance)) as u64;

    info!("💰 Trade parameters:");
    info!("   Input: {} lamports", position_size_lamports);
    info!("   Min output: {} lamports", min_amount_out);
    info!("   Direction: {} -> {}",
          if swap_for_y { "X" } else { "Y" },
          if swap_for_y { "Y" } else { "X" });

    // Derive reserve PDAs (standard derivation for Meteora)
    let (reserve_x, _) = Pubkey::find_program_address(
        &[b"reserve_x", lb_pair.as_ref()],
        &program_id,
    );

    let (reserve_y, _) = Pubkey::find_program_address(
        &[b"reserve_y", lb_pair.as_ref()],
        &program_id,
    );

    // Derive oracle PDA
    let (oracle, _) = Pubkey::find_program_address(
        &[b"oracle", lb_pair.as_ref()],
        &program_id,
    );

    // Build instruction data
    let swap_data = SwapInstructionData {
        amount_in: position_size_lamports,
        min_amount_out,
    };

    // Meteora swap discriminator (from IDL) - this is the instruction selector
    // For swap instruction, discriminator is typically the first 8 bytes of SHA256("global:swap")
    let mut data = vec![0xf8, 0xc6, 0x9e, 0x91, 0xe1, 0x75, 0x87, 0xc8];  // swap discriminator
    data.extend_from_slice(&swap_data.try_to_vec()?);

    // Build accounts array for swap instruction (OFFICIAL lb_clmm SDK order)
    // Reference: lb_clmm-0.1.1/src/instructions/swap.rs
    let accounts = vec![
        // Core accounts
        AccountMeta::new(lb_pair, false),                           // 0. lb_pair
        // Note: bin_array_bitmap_extension is optional, skip for now
        AccountMeta::new(reserve_x, false),                         // 1. reserve_x
        AccountMeta::new(reserve_y, false),                         // 2. reserve_y

        // User token accounts (in/out depend on swap direction)
        AccountMeta::new(if swap_for_y { user_token_x } else { user_token_y }, false), // 3. user_token_in
        AccountMeta::new(if swap_for_y { user_token_y } else { user_token_x }, false), // 4. user_token_out

        // Token mints (CRITICAL - these were missing!)
        AccountMeta::new_readonly(token_x_mint, false),             // 5. token_x_mint
        AccountMeta::new_readonly(token_y_mint, false),             // 6. token_y_mint

        // Oracle
        AccountMeta::new(oracle, false),                            // 7. oracle

        // Note: host_fee_in is optional, skip for now

        // Signer
        AccountMeta::new_readonly(*user_wallet, true),              // 8. user (signer)

        // Token programs (CRITICAL - these were missing!)
        AccountMeta::new_readonly(token_program_id(), false),       // 9. token_x_program
        AccountMeta::new_readonly(token_program_id(), false),       // 10. token_y_program
    ];

    let instruction = Instruction {
        program_id,
        accounts,
        data,
    };

    info!("✅ Meteora swap instruction built successfully");
    Ok(instruction)
}

/// Execute Meteora swap transaction
pub async fn execute_meteora_swap(
    rpc_client: Arc<SolanaRpcClient>,
    pool_address: &str,
    position_size_lamports: u64,
    user_keypair: &Keypair,
    slippage_tolerance: f64,
    swap_for_y: bool,
    cached_blockhash: Option<&crate::cached_blockhash::SharedCachedBlockhash>,
) -> Result<String> {
    info!("🚀 Executing Meteora swap...");

    // Build the swap instruction
    let swap_ix = build_meteora_swap_instruction(
        rpc_client.clone(),
        pool_address,
        position_size_lamports,
        &user_keypair.pubkey(),
        slippage_tolerance,
        swap_for_y,
    ).await?;

    // Create transaction
    let mut transaction = Transaction::new_with_payer(
        &[swap_ix],
        Some(&user_keypair.pubkey()),
    );

    // Get recent blockhash (use cached if available, otherwise fetch)
    let recent_blockhash = match cached_blockhash {
        Some(cache) => {
            crate::cached_blockhash::get_blockhash(cache, &rpc_client).await
                .map_err(|e| anyhow!("Failed to get blockhash: {}", e))?
        }
        None => {
            rpc_client.get_latest_blockhash()
                .map_err(|e| anyhow!("Failed to get blockhash: {}", e))?
        }
    };

    transaction.sign(&[user_keypair], recent_blockhash);

    // MANDATORY SIMULATION (Grok's safety recommendation)
    info!("🧪 Simulating transaction...");
    let simulation_success = rpc_client.simulate_transaction(&transaction)
        .map_err(|e| anyhow!("Simulation failed: {}", e))?;

    if !simulation_success {
        warn!("❌ Simulation failed - transaction would revert on-chain");
        return Err(anyhow!("Transaction would fail on-chain - simulation returned false"));
    }

    info!("✅ Simulation passed");

    // Send transaction
    info!("📡 Sending transaction to blockchain...");
    let signature = rpc_client.send_transaction(&transaction)
        .map_err(|e| anyhow!("Failed to send transaction: {}", e))?;

    info!("🎉 Swap executed successfully!");
    info!("   Signature: {}", signature);

    Ok(signature.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_swap_data_serialization() {
        let data = SwapInstructionData {
            amount_in: 1000000,
            min_amount_out: 950000,
        };

        let serialized = data.try_to_vec().unwrap();
        assert!(serialized.len() > 0);
    }
}
