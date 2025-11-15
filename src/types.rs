// Common types for DEX swap operations

use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

/// Type of DEX
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DexType {
    // Meteora variants
    MeteoraDammV1, // Meteora DAMM V1 (older version)
    MeteoraDammV2, // Meteora DAMM V2 (newer version)
    MeteoraDlmm,   // Meteora Dynamic Liquidity Market Maker

    // Orca variants
    OrcaWhirlpools, // Orca Whirlpools (concentrated liquidity)
    OrcaLegacy,     // Orca Legacy (older AMM)

    // Raydium variants (all use same program family)
    RaydiumAmmV4,  // Raydium AMM V4 (main AMM)
    RaydiumClmm,   // Raydium Concentrated Liquidity
    RaydiumCpmm,   // Raydium Constant Product Market Maker
    RaydiumStable, // Raydium Stable Swap

    // Other DEXes
    PumpSwap, // Post-migration Pump.fun tokens
    Jupiter,  // Jupiter Aggregator
    Serum,    // Serum Order Book DEX
    Aldrin,   // Aldrin AMM
    Saros,    // Saros AMM
    Crema,    // Crema Finance
    Cropper,  // Cropper Finance
    Lifinity, // Lifinity AMM
    Fluxbeam, // Fluxbeam DEX
    HumidiFi, // Dark pool/proprietary AMM - highest volume DEX on Solana
}

/// Pool information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolInfo {
    pub full_address: Pubkey,
    pub dex_type: DexType,
    pub token_a_mint: Pubkey,
    pub token_b_mint: Pubkey,
    pub reserve_a: Pubkey,
    pub reserve_b: Pubkey,
}

/// Swap parameters
#[derive(Debug, Clone)]
pub struct SwapParams {
    pub amount_in: u64,
    pub minimum_amount_out: u64,
    pub expected_amount_out: Option<u64>, // Expected output for slippage validation
    pub swap_a_to_b: bool,                // true = A→B, false = B→A
}

impl DexType {
    /// Parse from DEX string like "Meteora_DAMM_V2_81vA2wJx"
    pub fn from_dex_string(dex_str: &str) -> anyhow::Result<Self> {
        // Meteora variants
        if dex_str.starts_with("Meteora_DAMM_V1") {
            Ok(DexType::MeteoraDammV1)
        } else if dex_str.starts_with("Meteora_DAMM_V2") || dex_str.starts_with("Meteora_Pools") {
            Ok(DexType::MeteoraDammV2)
        } else if dex_str.starts_with("Meteora_DLMM") {
            Ok(DexType::MeteoraDlmm)

        // Orca variants
        } else if dex_str.starts_with("Orca_Whirlpools") {
            Ok(DexType::OrcaWhirlpools)
        } else if dex_str.starts_with("Orca_Legacy") {
            Ok(DexType::OrcaLegacy)

        // Raydium variants
        } else if dex_str.starts_with("Raydium_AMM_V4") || dex_str.starts_with("Raydium_AMM") {
            Ok(DexType::RaydiumAmmV4)
        } else if dex_str.starts_with("Raydium_CLMM") {
            Ok(DexType::RaydiumClmm)
        } else if dex_str.starts_with("Raydium_CPMM") {
            Ok(DexType::RaydiumCpmm)
        } else if dex_str.starts_with("Raydium_Stable") {
            Ok(DexType::RaydiumStable)

        // Other DEXes
        } else if dex_str.starts_with("PumpSwap") || dex_str.starts_with("Pump_Swap") {
            Ok(DexType::PumpSwap)
        } else if dex_str.starts_with("Jupiter") {
            Ok(DexType::Jupiter)
        } else if dex_str.starts_with("Serum") {
            Ok(DexType::Serum)
        } else if dex_str.starts_with("Aldrin") {
            Ok(DexType::Aldrin)
        } else if dex_str.starts_with("Saros") {
            Ok(DexType::Saros)
        } else if dex_str.starts_with("Crema") {
            Ok(DexType::Crema)
        } else if dex_str.starts_with("Cropper") {
            Ok(DexType::Cropper)
        } else if dex_str.starts_with("Lifinity") {
            Ok(DexType::Lifinity)
        } else if dex_str.starts_with("Fluxbeam") {
            Ok(DexType::Fluxbeam)
        } else if dex_str.starts_with("HumidiFi") || dex_str.starts_with("Humidifi") {
            Ok(DexType::HumidiFi)
        } else {
            Err(anyhow::anyhow!("Unknown DEX type: {}", dex_str))
        }
    }
}

/// Extract short pool ID from DEX string
pub fn extract_pool_id(dex_str: &str) -> anyhow::Result<String> {
    let parts: Vec<&str> = dex_str.split('_').collect();
    parts
        .last()
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("Invalid DEX string format: {}", dex_str))
}
