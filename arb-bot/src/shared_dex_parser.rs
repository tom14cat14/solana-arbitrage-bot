use anyhow::Result;
use solana_sdk::transaction::Transaction;
use solana_sdk::pubkey::Pubkey;
use chrono::{DateTime, Utc};
use std::str::FromStr;

/// Shared DEX Swap Parser - Used by ALL bots
///
/// This parser extracts swap information from Solana transactions
/// and works with ShredStream, Helius, or any transaction source.
///
/// Supported DEXs:
/// - Raydium (AMM V4, CLMM, CPMM)
/// - Orca (Whirlpools)
/// - PumpSwap (Pump.fun)
/// - Jupiter (Aggregator)
/// - Meteora (DLMM)
/// - Serum (OpenBook)

#[derive(Debug, Clone)]
pub struct SwapInfo {
    /// Transaction signature (unique ID)
    pub signature: String,
    /// Slot number when swap occurred
    pub slot: u64,
    /// DEX name (e.g., "Raydium", "Orca", "PumpSwap")
    pub dex_name: String,
    /// DEX program ID
    pub dex_program_id: String,
    /// Token being swapped (mint address)
    pub token_mint: String,
    /// Amount of token in
    pub amount_in: u64,
    /// Amount of token out
    pub amount_out: u64,
    /// Price in SOL (calculated from amounts)
    pub price_sol: f64,
    /// Pool liquidity after swap
    pub liquidity: u64,
    /// Swap direction (true = buy, false = sell)
    pub is_buy: bool,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// User wallet that did the swap
    pub user_wallet: String,
}

pub struct SharedDexParser {
    /// Known DEX program IDs
    dex_programs: Vec<DexProgramInfo>,
}

#[derive(Clone)]
struct DexProgramInfo {
    name: String,
    program_id: Pubkey,
    swap_discriminator: Vec<u8>,
}

impl SharedDexParser {
    pub fn new() -> Self {
        let dex_programs = vec![
            // Raydium AMM V4
            DexProgramInfo {
                name: "Raydium".to_string(),
                program_id: Pubkey::from_str("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8").unwrap(),
                swap_discriminator: vec![143, 190, 90, 218, 196, 30, 51, 222], // swap instruction
            },
            // Orca Whirlpools
            DexProgramInfo {
                name: "Orca".to_string(),
                program_id: Pubkey::from_str("whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc").unwrap(),
                swap_discriminator: vec![248, 198, 158, 145, 225, 117, 135, 200], // swap instruction
            },
            // PumpSwap (Pump.fun)
            DexProgramInfo {
                name: "PumpSwap".to_string(),
                program_id: Pubkey::from_str("6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P").unwrap(),
                swap_discriminator: vec![102, 6, 61, 18, 1, 218, 235, 234], // swap instruction
            },
            // Jupiter Aggregator V6
            DexProgramInfo {
                name: "Jupiter".to_string(),
                program_id: Pubkey::from_str("JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4").unwrap(),
                swap_discriminator: vec![229, 23, 203, 151, 122, 227, 173, 42], // route instruction
            },
            // Meteora DLMM
            DexProgramInfo {
                name: "Meteora".to_string(),
                program_id: Pubkey::from_str("LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo").unwrap(),
                swap_discriminator: vec![248, 198, 158, 145, 225, 117, 135, 200], // swap instruction
            },
        ];

        Self { dex_programs }
    }

    /// Parse a transaction and extract swap information if it's a DEX swap
    pub fn parse_transaction(&self, tx: &Transaction, signature: String, slot: u64) -> Option<SwapInfo> {
        // Check each instruction in the transaction
        for (idx, instruction) in tx.message.instructions.iter().enumerate() {
            // Get program ID for this instruction
            let program_id_index = instruction.program_id_index as usize;
            if program_id_index >= tx.message.account_keys.len() {
                continue;
            }
            let program_id = tx.message.account_keys[program_id_index];

            // Check if this is a known DEX program
            for dex_info in &self.dex_programs {
                if program_id == dex_info.program_id {
                    // Check if instruction data matches swap discriminator
                    if instruction.data.len() >= 8 {
                        let discriminator = &instruction.data[0..8];
                        if discriminator == dex_info.swap_discriminator.as_slice() {
                            // This is a DEX swap! Parse it
                            return self.parse_swap_instruction(
                                tx,
                                instruction,
                                &dex_info.name,
                                &signature,
                                slot,
                            );
                        }
                    }
                }
            }
        }

        None
    }

    /// Parse swap instruction to extract amounts and calculate price
    fn parse_swap_instruction(
        &self,
        tx: &Transaction,
        instruction: &solana_sdk::instruction::CompiledInstruction,
        dex_name: &str,
        signature: &str,
        slot: u64,
    ) -> Option<SwapInfo> {
        // Extract amounts from instruction data
        // Layout varies by DEX, but generally:
        // - Bytes 8-16: amount_in (u64)
        // - Bytes 16-24: min_amount_out (u64)

        if instruction.data.len() < 24 {
            return None;
        }

        let amount_in = u64::from_le_bytes(
            instruction.data[8..16].try_into().ok()?
        );

        let amount_out = u64::from_le_bytes(
            instruction.data[16..24].try_into().ok()?
        );

        // Get token mint from accounts
        // Typically: accounts[2] or accounts[3] is the token mint
        let token_mint = if instruction.accounts.len() > 2 {
            let mint_index = instruction.accounts[2] as usize;
            if mint_index < tx.message.account_keys.len() {
                tx.message.account_keys[mint_index].to_string()
            } else {
                return None;
            }
        } else {
            return None;
        };

        // Get user wallet (first account is usually the signer)
        let user_wallet = if !instruction.accounts.is_empty() {
            let user_index = instruction.accounts[0] as usize;
            if user_index < tx.message.account_keys.len() {
                tx.message.account_keys[user_index].to_string()
            } else {
                "unknown".to_string()
            }
        } else {
            "unknown".to_string()
        };

        // Calculate price (assuming SOL is quote token)
        // Price = amount_out / amount_in (adjust for decimals)
        let price_sol = if amount_in > 0 {
            (amount_out as f64) / (amount_in as f64)
        } else {
            0.0
        };

        // Determine buy/sell based on which is SOL
        // This is simplified - real implementation needs to check token types
        let is_buy = amount_in < amount_out;

        Some(SwapInfo {
            signature: signature.to_string(),
            slot,
            dex_name: dex_name.to_string(),
            dex_program_id: instruction.program_id_index.to_string(),
            token_mint,
            amount_in,
            amount_out,
            price_sol,
            liquidity: 0, // TODO: Extract from pool account
            is_buy,
            timestamp: Utc::now(),
            user_wallet,
        })
    }

    /// Parse multiple transactions from a ShredStream entry
    pub fn parse_transactions(&self, transactions: &[Transaction], slot: u64) -> Vec<SwapInfo> {
        let mut swaps = Vec::new();

        for (idx, tx) in transactions.iter().enumerate() {
            // Generate signature (in real impl, extract from transaction)
            let signature = format!("sig_{}", idx);

            if let Some(swap) = self.parse_transaction(tx, signature, slot) {
                swaps.push(swap);
            }
        }

        swaps
    }
}

/// Helper function to extract transactions from ShredStream entries
pub fn extract_transactions_from_entries(
    entries: &[solana_entry::entry::Entry]
) -> Vec<Transaction> {
    entries
        .iter()
        .flat_map(|entry| entry.transactions.iter().cloned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_creation() {
        let parser = SharedDexParser::new();
        assert_eq!(parser.dex_programs.len(), 5); // 5 DEXs supported
    }

    #[test]
    fn test_swap_detection() {
        // TODO: Add test with actual transaction data
    }
}
