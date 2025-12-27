use anyhow::Result;
use arb_bot::dex_registry::{DexRegistry, DexInfo};
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

#[test]
fn test_dex_registry_initialization() {
    let registry = DexRegistry::new();

    // Should have all major DEXs loaded
    assert!(registry.dexs.len() >= 15);

    // Check for specific DEX entries
    let required_dexs = vec![
        "Raydium_AMM_V4",
        "Raydium_CLMM",
        "Orca_Whirlpools",
        "Jupiter",
        "Serum",
        "Meteora_DAMM",
        "PumpFun",
        "Aldrin",
        "Crema",
        "Lifinity",
    ];

    for dex_name in required_dexs {
        assert!(registry.dexs.contains_key(dex_name), "Missing DEX: {}", dex_name);
    }
}

#[test]
fn test_dex_info_properties() {
    let registry = DexRegistry::new();

    if let Some(raydium) = registry.dexs.get("Raydium_AMM_V4") {
        assert_eq!(raydium.name, "Raydium_AMM_V4");
        assert!(raydium.fee_rate > 0.0 && raydium.fee_rate < 0.01); // Reasonable fee range
        assert!(!raydium.program_id.to_string().is_empty());
        assert!(raydium.supported_tokens > 0);
        assert!(raydium.liquidity > 0.0);
        assert!(raydium.supports_arbitrage);
    } else {
        panic!("Raydium_AMM_V4 not found in registry");
    }
}

#[test]
fn test_program_id_lookups() {
    let registry = DexRegistry::new();

    // Test known program IDs
    let raydium_program_id = Pubkey::from_str("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8").unwrap();
    let orca_program_id = Pubkey::from_str("whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc").unwrap();

    // These might not work yet since the methods have dead code warnings
    // But the structure should be there
    assert!(registry.dexs.values().any(|dex| dex.program_id == raydium_program_id));
    assert!(registry.dexs.values().any(|dex| dex.program_id == orca_program_id));
}

#[test]
fn test_fee_calculations() {
    let registry = DexRegistry::new();

    // Test that all DEXs have reasonable fee rates
    for (_name, dex_info) in &registry.dexs {
        assert!(dex_info.fee_rate >= 0.0, "Fee rate cannot be negative for {}", dex_info.name);
        assert!(dex_info.fee_rate <= 0.01, "Fee rate too high for {}: {}", dex_info.name, dex_info.fee_rate);
    }
}

#[test]
fn test_liquidity_thresholds() {
    let registry = DexRegistry::new();

    // Test that major DEXs have substantial liquidity
    let major_dexs = vec!["Raydium_AMM_V4", "Orca_Whirlpools", "Jupiter"];

    for dex_name in major_dexs {
        if let Some(dex_info) = registry.dexs.get(dex_name) {
            assert!(dex_info.liquidity > 1_000_000.0,
                   "Major DEX {} should have substantial liquidity: {}",
                   dex_name, dex_info.liquidity);
        }
    }
}

#[test]
fn test_arbitrage_support() {
    let registry = DexRegistry::new();

    // Most DEXs should support arbitrage
    let arbitrage_supporting_count = registry.dexs.values()
        .filter(|dex| dex.supports_arbitrage)
        .count();

    assert!(arbitrage_supporting_count >= 10,
           "Should have at least 10 DEXs supporting arbitrage, found: {}",
           arbitrage_supporting_count);
}

#[test]
fn test_mev_protection_flags() {
    let registry = DexRegistry::new();

    // Some DEXs should have MEV protection
    let mev_protected_count = registry.dexs.values()
        .filter(|dex| dex.supports_mev_protection)
        .count();

    assert!(mev_protected_count > 0, "Should have some DEXs with MEV protection");

    // PumpFun specifically should have MEV protection due to sniping concerns
    if let Some(pumpfun) = registry.dexs.get("PumpFun") {
        assert!(pumpfun.supports_mev_protection, "PumpFun should have MEV protection");
    }
}

#[test]
fn test_supported_tokens_counts() {
    let registry = DexRegistry::new();

    for (_name, dex_info) in &registry.dexs {
        assert!(dex_info.supported_tokens > 0,
               "DEX {} should support at least 1 token", dex_info.name);

        // Major DEXs should support many tokens
        if ["Raydium_AMM_V4", "Orca_Whirlpools", "Jupiter"].contains(&dex_info.name.as_str()) {
            assert!(dex_info.supported_tokens >= 100,
                   "Major DEX {} should support many tokens: {}",
                   dex_info.name, dex_info.supported_tokens);
        }
    }
}

#[test]
fn test_dex_categories() {
    let registry = DexRegistry::new();

    // Count different types of DEXs
    let mut amm_count = 0;
    let mut clmm_count = 0;
    let mut orderbook_count = 0;
    let mut aggregator_count = 0;

    for (_name, dex_info) in &registry.dexs {
        if dex_info.name.contains("AMM") {
            amm_count += 1;
        }
        if dex_info.name.contains("CLMM") || dex_info.name.contains("Whirlpools") {
            clmm_count += 1;
        }
        if dex_info.name.contains("Serum") {
            orderbook_count += 1;
        }
        if dex_info.name.contains("Jupiter") {
            aggregator_count += 1;
        }
    }

    // Should have variety of DEX types
    assert!(amm_count > 0, "Should have AMM DEXs");
    assert!(clmm_count > 0, "Should have CLMM DEXs");
    assert!(orderbook_count > 0, "Should have orderbook DEXs");
    assert!(aggregator_count > 0, "Should have aggregator DEXs");
}

#[test]
fn test_unique_program_ids() {
    let registry = DexRegistry::new();

    let mut program_ids = std::collections::HashSet::new();

    for (_name, dex_info) in &registry.dexs {
        assert!(program_ids.insert(dex_info.program_id.to_string()),
               "Duplicate program ID found: {} for DEX: {}",
               dex_info.program_id, dex_info.name);
    }
}

#[test]
fn test_arbitrage_pair_combinations() {
    let registry = DexRegistry::new();

    // Get arbitrage-supporting DEXs
    let arbitrage_dexs: Vec<&DexInfo> = registry.dexs.values()
        .filter(|dex| dex.supports_arbitrage)
        .collect();

    // Should be able to form multiple arbitrage pairs
    let possible_pairs = arbitrage_dexs.len() * (arbitrage_dexs.len() - 1) / 2;
    assert!(possible_pairs >= 45, "Should have many possible arbitrage pairs: {}", possible_pairs);
}

#[test]
fn test_performance_characteristics() {
    let registry = DexRegistry::new();

    // Fast DEXs for arbitrage
    let fast_dexs = vec!["Raydium_AMM_V4", "Orca_Whirlpools"];

    for dex_name in fast_dexs {
        if let Some(dex_info) = registry.dexs.get(dex_name) {
            // These should be marked as supporting arbitrage due to speed
            assert!(dex_info.supports_arbitrage,
                   "Fast DEX {} should support arbitrage", dex_name);
        }
    }
}