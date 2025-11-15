// Helper module for populating pool registry with known pools
//
// This is a temporary solution until we have dynamic pool discovery
// Options for future improvement:
// 1. Query ShredStream service API for full addresses
// 2. Query Meteora API for all pools
// 3. Use getProgramAccounts (slow)
// 4. Enhance ShredStream service to provide full addresses

use anyhow::Result;
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use tracing::{info, warn};

use crate::{PoolRegistry, PoolInfo, DexType};

/// Populate pool registry with known Meteora DLMM pools
///
/// This function adds known pool addresses that we've seen in live trading.
/// As we discover more pools through live data, we can add them here.
///
/// **CRITICAL**: You MUST populate this with actual pool addresses before live trading!
/// The pool IDs shown in ShredStream are just 8-char prefixes, not full addresses.
pub fn populate_known_pools(pool_registry: Arc<PoolRegistry>) -> Result<()> {
    info!("📋 Populating pool registry with known Meteora DLMM pools...");

    // Top liquidity Meteora DLMM pools (queried from https://dlmm-api.meteora.ag/pair/all_by_groups)
    // Updated: 2025-10-06

    // 1. SOL-USDC (High Liquidity #1)
    pool_registry.register_pool(
        "BGm1tav5".to_string(),
        PoolInfo {
            full_address: "BGm1tav58oGcsQJehL9WXBFXF7D27vZsKefj4xJKD5Y".parse()?,
            dex_type: DexType::MeteoraDammV2,
            token_a_mint: "So11111111111111111111111111111111111111112".parse()?,  // SOL
            token_b_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".parse()?,  // USDC
            reserve_a: "DwZz4S1Z1LBXomzmncQRVKCYhjCqSAMQ6RPKbUAadr7H".parse()?,
            reserve_b: "4N22J4vW2juHocTntJNmXywSonYjkndCwahjZ2cYLDgb".parse()?,
        }
    )?;

    // 2. JitoSOL-SOL (High Liquidity #2)
    pool_registry.register_pool(
        "BoeMUkCL".to_string(),
        PoolInfo {
            full_address: "BoeMUkCLHchTD31HdXsbDExuZZfcUppSLpYtV3LZTH6U".parse()?,
            dex_type: DexType::MeteoraDammV2,
            token_a_mint: "J1toso1uCk3RLmjorhTtrVwY9HJ7X8V9yYac6Y7kGCPn".parse()?,  // JitoSOL
            token_b_mint: "So11111111111111111111111111111111111111112".parse()?,  // SOL
            reserve_a: "93d6ukn24o1xMcMDip2SACKG8GbvhGUZim1e3ZEcQVm2".parse()?,
            reserve_b: "CodroyzrRNvc5kHRoAQYjpVSr1jA9fLcUWVFouiuWGsD".parse()?,
        }
    )?;

    // 3. SOL-USDC (High Liquidity #3)
    pool_registry.register_pool(
        "BVRbyLjj".to_string(),
        PoolInfo {
            full_address: "BVRbyLjjfSBcoyiYFuxbgKYnWuiFaF9CSXEa5vdSZ9Hh".parse()?,
            dex_type: DexType::MeteoraDammV2,
            token_a_mint: "So11111111111111111111111111111111111111112".parse()?,  // SOL
            token_b_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".parse()?,  // USDC
            reserve_a: "FMzVsENjscefpAtUJYBUTeJAYaKNfFQBHjTZE1AQRFYY".parse()?,
            reserve_b: "7du3jFJK4rhf9JnZSQmhr6qPkgdQyJ88528qyxpYPPtL".parse()?,
        }
    )?;

    // 4. SOL-USDC (High Liquidity #4)
    pool_registry.register_pool(
        "HTvjzsf".to_string(),
        PoolInfo {
            full_address: "HTvjzsfX3yU6BUodCjZ5vZkUrAxMDTrBs3CJaq43ashR".parse()?,
            dex_type: DexType::MeteoraDammV2,
            token_a_mint: "So11111111111111111111111111111111111111112".parse()?,  // SOL
            token_b_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".parse()?,  // USDC
            reserve_a: "H7j5NPopj3tQvDg4N8CxwtYciTn3e8AEV6wSVrxpyDUc".parse()?,
            reserve_b: "HbYjRzx7teCxqW3unpXBEcNHhfVZvW2vW9MQ99TkizWt".parse()?,
        }
    )?;

    // 5. SOL-USDC (High Liquidity #5)
    pool_registry.register_pool(
        "5rCf1DM8".to_string(),
        PoolInfo {
            full_address: "5rCf1DM8LjKTw4YqhnoLcngyZYeNnQqztScTogYHAS6".parse()?,
            dex_type: DexType::MeteoraDammV2,
            token_a_mint: "So11111111111111111111111111111111111111112".parse()?,  // SOL
            token_b_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".parse()?,  // USDC
            reserve_a: "EYj9xKw6ZszwpyNibHY7JD5o3QgTVrSdcBp1fMJhrR9o".parse()?,
            reserve_b: "CoaxzEh8p5YyGLcj36Eo3cUThVJxeKCs7qvLAGDYwBcz".parse()?,
        }
    )?;

    // 6. JUP-SOL
    pool_registry.register_pool(
        "C8Gr6AUu".to_string(),
        PoolInfo {
            full_address: "C8Gr6AUuq9hEdSYJzoEpNcdjpojPZwqG5MtQbeouNNwg".parse()?,
            dex_type: DexType::MeteoraDammV2,
            token_a_mint: "JUPyiwrYJFskUPiHa7hkeR8VUtAeFoSYbKedZNsDvCN".parse()?,  // JUP
            token_b_mint: "So11111111111111111111111111111111111111112".parse()?,  // SOL
            reserve_a: "37XRwFkmrvrh57MuyHJ651qwXikmsUbcH29Uj5USWq1E".parse()?,
            reserve_b: "5rJ5PvB5MyxsyV9VSid2esNLJUykRiq9xcGxnMmoDJhh".parse()?,
        }
    )?;

    // 7. cbBTC-SOL
    pool_registry.register_pool(
        "7wJK6JJQ".to_string(),
        PoolInfo {
            full_address: "7wJK6JJQERsyRoDNVnbkDtBKbXfoBV2dw8uP45WD5aC1".parse()?,
            dex_type: DexType::MeteoraDammV2,
            token_a_mint: "cbbtcf3aa214zXHbiAZQwf4122FBYbraNdFqgw4iMij".parse()?,  // cbBTC
            token_b_mint: "So11111111111111111111111111111111111111112".parse()?,  // SOL
            reserve_a: "82gYLm4jD9N6YXU86UJZQ5ziGbNBxpxNgmpe3TNP2Bgr".parse()?,
            reserve_b: "8q5Cpus9iyPRp7KCxFFHJ3fcUcaMtadhzJ2S3YZA1VJ6".parse()?,
        }
    )?;

    // 8. USDC-USDT (Stablecoin pair for arbitrage)
    pool_registry.register_pool(
        "ARwi1S4D".to_string(),
        PoolInfo {
            full_address: "ARwi1S4DaiTG5DX7S4M4ZsrXqpMD1MrTmbu9ue2tpmEq".parse()?,
            dex_type: DexType::MeteoraDammV2,
            token_a_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".parse()?,  // USDC
            token_b_mint: "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB".parse()?,  // USDT
            reserve_a: "4STreSrMtf8umxyei9DaZG4bX3HT9hE3TGw3Xz41XNHd".parse()?,
            reserve_b: "GkTrsQsu8WvrbairmN12aUKk74qHivRNFxaT5YxCECKQ".parse()?,
        }
    )?;

    // 9. JupSOL-SOL
    pool_registry.register_pool(
        "bNcdL9Hy".to_string(),
        PoolInfo {
            full_address: "bNcdL9Hy85c9qb4hRavAUFtJUiyRPh3u96jerFqZQq6".parse()?,
            dex_type: DexType::MeteoraDammV2,
            token_a_mint: "jupSoLaHXQiZZTSfEWMTRRgpnyFm8f6sZdosWBjx93v".parse()?,  // jupSOL
            token_b_mint: "So11111111111111111111111111111111111111112".parse()?,  // SOL
            reserve_a: "2eF8kcFF6musyQQMckCDriXpirZW6vocJeh6q1noXcNW".parse()?,
            reserve_b: "HTeD5fFp1oCvnNioZFQgXAfuRDzHWpDQS5y7NvsopKXN".parse()?,
        }
    )?;

    // 10. PUMP-USDC
    pool_registry.register_pool(
        "9SMp4yLK".to_string(),
        PoolInfo {
            full_address: "9SMp4yLKGtW9TnLimfVPkDARsyNSfJw43WMke4r7KoZj".parse()?,
            dex_type: DexType::MeteoraDammV2,
            token_a_mint: "pumpCmXqMfrsAkQ5r49WcJnRayYRqmXz6ae8H7H9Dfn".parse()?,  // PUMP
            token_b_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".parse()?,  // USDC
            reserve_a: "6uVEyA1RRhuTzDroFGBrDsAHwE4b6hCSwgyXAHjTZEUv".parse()?,
            reserve_b: "5RLzTiyGuadAC4SE3s7MGonXszFShJtZewVmmHGUUbkV".parse()?,
        }
    )?;

    // 11. PUMP-SOL
    pool_registry.register_pool(
        "HbjYfcWZ".to_string(),
        PoolInfo {
            full_address: "HbjYfcWZBjCBYTJpZkLGxqArVmZVu3mQcRudb6Wg1sVh".parse()?,
            dex_type: DexType::MeteoraDammV2,
            token_a_mint: "pumpCmXqMfrsAkQ5r49WcJnRayYRqmXz6ae8H7H9Dfn".parse()?,  // PUMP
            token_b_mint: "So11111111111111111111111111111111111111112".parse()?,  // SOL
            reserve_a: "5uXsebqNi3jDBvHvLJUuLqouUEHyQNDZcREHpLSwCZpM".parse()?,
            reserve_b: "CD1RxU49jNwxD7LvRvrdWDNLpx5ZrJ7khMEzTNudk94s".parse()?,
        }
    )?;

    info!("✅ Registered {} Meteora DLMM pools", 11);
    info!("   Including: SOL-USDC, JitoSOL-SOL, JUP-SOL, cbBTC-SOL, USDC-USDT, and more");
    info!("   All pools have high liquidity and active trading");

    Ok(())
}

/// Attempt to resolve pool address from ShredStream service
///
/// This makes an HTTP request to the ShredStream service asking for the full address.
/// Requires ShredStream service to be enhanced with this endpoint.
#[allow(dead_code)]
pub async fn resolve_pool_from_shredstream(
    shredstream_url: &str,
    short_id: &str,
) -> Result<Pubkey> {
    let url = format!("{}/api/pool/{}", shredstream_url, short_id);

    info!("🔍 Querying ShredStream for pool: {}", short_id);

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await?;

    if response.status().is_success() {
        let full_address: String = response.json().await?;
        info!("✅ Resolved {} → {}", short_id, full_address);
        Ok(full_address.parse()?)
    } else {
        Err(anyhow::anyhow!(
            "ShredStream service returned error {} for pool {}",
            response.status(),
            short_id
        ))
    }
}

/// Query Meteora API for all DLMM pools
///
/// This can be used to populate the registry dynamically.
/// Run this once and cache the results.
#[allow(dead_code)]
pub async fn fetch_meteora_pools() -> Result<Vec<(String, Pubkey)>> {
    info!("🔍 Fetching all Meteora DLMM pools from API...");

    // Meteora API endpoint (example - verify actual endpoint)
    let url = "https://dlmm-api.meteora.ag/pair/all";

    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "Meteora API returned error: {}",
            response.status()
        ));
    }

    // Parse response and extract pool addresses
    // Format depends on Meteora API response structure
    warn!("⚠️ Meteora API parsing not yet implemented");
    warn!("   Check Meteora documentation for API format");

    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_populate_compiles() {
        // Just verify the function compiles
        // Can't actually test without real pool data
    }
}
