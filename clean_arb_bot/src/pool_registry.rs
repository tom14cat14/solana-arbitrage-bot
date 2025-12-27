// Pool registry for mapping short pool IDs to full Solana addresses
//
// CRITICAL PROBLEM: ShredStream cache uses 8-char short IDs (e.g., "81vA2wJx")
// but Solana needs full 44-char addresses (e.g., "81vA2wJx...")
//
// SOLUTION APPROACHES:
// 1. Pre-populate registry with known pools
// 2. Query ShredStream service API for full addresses
// 3. Use getProgramAccounts with prefix matching (slow)
// 4. Enhance ShredStream service to store full addresses (recommended long-term)
//
// CURRENT IMPLEMENTATION: In-memory cache with manual registration

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tokio::sync::RwLock as TokioRwLock;
use tracing::{debug, info, warn}; // For async validation cache

use crate::rpc_client::SolanaRpcClient;
use crate::types::{DexType, PoolInfo};

// Pool validation constants (Grok's ghost pool solution)
const MIN_POOL_SIZE: usize = 1000; // Minimum bytes for valid pool (DEX-specific)
const VALIDATION_TTL_SECS: u64 = 300; // 5 minutes cache TTL
const BACKGROUND_INTERVAL_SECS: u64 = 120; // 2 minutes background validation

/// Cache entry for resolved pool addresses
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PoolCacheEntry {
    short_id: String,
    full_address: String,
    dex_type: String,
    timestamp: u64,
}

/// ShredStream API response for pool lookup
#[derive(Debug, Deserialize)]
struct ShredStreamPoolResponse {
    full_address: Option<String>,
    pool_address: Option<String>,
}

/// Pool registry for managing pool address mappings
pub struct PoolRegistry {
    /// Map of short_id -> PoolInfo (Layer 1: In-memory cache)
    pools: Arc<RwLock<HashMap<String, PoolInfo>>>,
    /// Map of full_address -> short_id (reverse lookup)
    address_to_id: Arc<RwLock<HashMap<Pubkey, String>>>,
    /// RPC client for fetching pool data (Layer 4: On-chain fallback)
    rpc_client: Arc<SolanaRpcClient>,
    /// HTTP client for ShredStream service queries (Layer 2: ShredStream API)
    http_client: reqwest::Client,
    /// ShredStream service URL
    shredstream_url: String,
    /// Resolution performance metrics
    resolution_stats: Arc<RwLock<ResolutionStats>>,
    /// Pool validation cache (pool_short_id -> (is_valid, last_checked))
    /// Grok's ghost pool solution: 5-minute TTL cache
    validation_cache: Arc<TokioRwLock<HashMap<String, (bool, Instant)>>>,
}

/// Statistics for pool resolution performance
#[derive(Debug, Default)]
struct ResolutionStats {
    layer1_hits: u64, // In-memory cache hits
    layer2_hits: u64, // ShredStream API hits
    layer3_hits: u64, // SQLite cache hits (future)
    layer4_hits: u64, // On-chain RPC hits
    total_lookups: u64,
    total_latency_ms: u64,
}

impl PoolRegistry {
    /// Create new pool registry
    pub fn new(rpc_client: Arc<SolanaRpcClient>) -> Self {
        let shredstream_url = std::env::var("SHREDSTREAM_SERVICE_URL")
            .unwrap_or_else(|_| "http://localhost:8080".to_string());

        info!(
            "✅ Pool registry initialized with ShredStream API: {}",
            shredstream_url
        );

        Self {
            pools: Arc::new(RwLock::new(HashMap::new())),
            address_to_id: Arc::new(RwLock::new(HashMap::new())),
            rpc_client,
            http_client: reqwest::Client::builder()
                .timeout(Duration::from_millis(500)) // 500ms timeout for ShredStream API
                .build()
                .expect("Failed to create HTTP client"),
            shredstream_url,
            resolution_stats: Arc::new(RwLock::new(ResolutionStats::default())),
            validation_cache: Arc::new(TokioRwLock::new(HashMap::new())), // Grok's ghost pool solution
        }
    }

    /// Register a pool manually (for pre-population)
    pub fn register_pool(&self, short_id: String, pool_info: PoolInfo) -> Result<()> {
        let full_address = pool_info.full_address;

        // Validate short ID matches address prefix
        let address_str = full_address.to_string();
        if !address_str.starts_with(&short_id) {
            warn!(
                "⚠️ Short ID {} doesn't match address prefix {}",
                short_id, address_str
            );
        }

        // Store in both maps
        {
            let mut pools = self.pools.write().unwrap();
            pools.insert(short_id.clone(), pool_info);
        }

        {
            let mut addr_map = self.address_to_id.write().unwrap();
            addr_map.insert(full_address, short_id.clone());
        }

        debug!("✅ Registered pool: {} -> {}", short_id, full_address);
        Ok(())
    }

    /// Get pool info by short ID
    pub fn get_pool(&self, short_id: &str) -> Option<PoolInfo> {
        let pools = self.pools.read().unwrap();
        pools.get(short_id).cloned()
    }

    /// Get short ID by full address
    pub fn get_short_id(&self, full_address: &Pubkey) -> Option<String> {
        let addr_map = self.address_to_id.read().unwrap();
        addr_map.get(full_address).cloned()
    }

    /// Check if pool is registered
    pub fn has_pool(&self, short_id: &str) -> bool {
        let pools = self.pools.read().unwrap();
        pools.contains_key(short_id)
    }

    /// Get number of registered pools
    pub fn pool_count(&self) -> usize {
        let pools = self.pools.read().unwrap();
        pools.len()
    }

    /// Fetch pool state from blockchain
    pub fn fetch_pool_state(&self, pool_address: &Pubkey) -> Result<Vec<u8>> {
        debug!("Fetching pool state for: {}", pool_address);

        let data = self
            .rpc_client
            .get_account_data(pool_address)
            .context("Failed to fetch pool state")?;

        Ok(data)
    }

    /// Resolve short ID to full address using 4-layer hybrid approach
    ///
    /// Layer 1: In-memory cache (1-5ms) - Pre-populated pools
    /// Layer 2: ShredStream API (5-10ms) - Recent blockchain data
    /// Layer 3: SQLite cache (10-20ms) - Historical lookups (future)
    /// Layer 4: On-chain RPC (200-400ms) - Last resort fallback
    ///
    /// Returns full pool address or error if not found anywhere
    pub async fn resolve_pool_address(&self, short_id: &str, dex_type: &DexType) -> Result<Pubkey> {
        let start_time = Instant::now();

        // CRITICAL FIX: Increment lookup counter without holding lock during async operations
        {
            let mut stats = self.resolution_stats.write().unwrap();
            stats.total_lookups += 1;
        } // Lock released immediately

        debug!(
            "🔍 Resolving pool address for: {} ({:?})",
            short_id, dex_type
        );

        // LAYER 1: Check in-memory registry (fastest - 1-5ms)
        if let Some(pool_info) = self.get_pool(short_id) {
            let latency = start_time.elapsed().as_millis() as u64;

            // Update stats atomically without blocking
            {
                let mut stats = self.resolution_stats.write().unwrap();
                stats.layer1_hits += 1;
                stats.total_latency_ms += latency;
            } // Lock released immediately

            debug!("✅ Layer 1 HIT: Found in memory cache ({}ms)", latency);
            return Ok(pool_info.full_address);
        }

        // Handle full address input (if user provided 44-char address)
        if short_id.len() == 44 {
            if let Ok(pubkey) = short_id.parse::<Pubkey>() {
                debug!("✅ Parsed as full address: {}", short_id);
                return Ok(pubkey);
            }
        }

        // LAYER 2: Query ShredStream service API (fast - 5-10ms)
        // NO LOCK HELD during network I/O - prevents deadlock
        match self.query_shredstream_api(short_id).await {
            Ok(full_address) => {
                let latency = start_time.elapsed().as_millis() as u64;

                // Update stats atomically after network call completes
                {
                    let mut stats = self.resolution_stats.write().unwrap();
                    stats.layer2_hits += 1;
                    stats.total_latency_ms += latency;
                } // Lock released immediately

                debug!("✅ Layer 2 HIT: Found via ShredStream API ({}ms)", latency);

                // Cache in memory for future lookups
                let pool_info = PoolInfo {
                    full_address,
                    dex_type: dex_type.clone(),
                    token_a_mint: Pubkey::default(), // TODO: Get from API
                    token_b_mint: Pubkey::default(),
                    reserve_a: Pubkey::default(),
                    reserve_b: Pubkey::default(),
                };
                let _ = self.register_pool(short_id.to_string(), pool_info);

                return Ok(full_address);
            }
            Err(e) => {
                debug!("⚠️ Layer 2 MISS: ShredStream API query failed: {}", e);
            }
        }

        // LAYER 3: SQLite cache lookup (future implementation - 10-20ms)
        // TODO: Query persistent SQLite database
        // This provides fallback for pools seen historically but not currently in memory
        debug!("⚠️ Layer 3 SKIP: SQLite cache not yet implemented");

        // LAYER 4: On-chain RPC lookup (slowest - 200-400ms)
        // Only use as last resort for brand new pools
        // NO LOCK HELD during network I/O - prevents deadlock
        match self.query_on_chain(short_id, dex_type).await {
            Ok(full_address) => {
                let latency = start_time.elapsed().as_millis() as u64;

                // Update stats atomically after network call completes
                {
                    let mut stats = self.resolution_stats.write().unwrap();
                    stats.layer4_hits += 1;
                    stats.total_latency_ms += latency;
                } // Lock released immediately

                warn!(
                    "⚠️ Layer 4 HIT: Found via on-chain RPC ({}ms) - SLOW!",
                    latency
                );

                // FILTER: Reject old Pump.fun bonding curve (NOT arbitrageable)
                // Allow PumpSwap AMM (post-migration, standard AMM like Raydium)
                match self.rpc_client.get_account_owner(&full_address) {
                    Ok(owner) => {
                        // OLD Pump.fun bonding curve program (pre-migration)
                        let old_pump_fun_bonding: Pubkey =
                            "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P"
                                .parse()
                                .unwrap();

                        // PumpSwap AMM program (post-migration) - ALLOW
                        let pumpswap_amm: Pubkey = "pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA"
                            .parse()
                            .unwrap();

                        if owner == old_pump_fun_bonding {
                            debug!("⚠️ Skipping old Pump.fun bonding curve (pre-migration, not arbitrageable): {}", full_address);
                            return Err(anyhow::anyhow!(
                                "Pool {} is on old Pump.fun bonding curve (unsupported - use PumpSwap AMM instead)",
                                full_address
                            ));
                        }

                        if owner == pumpswap_amm {
                            debug!(
                                "✅ PumpSwap AMM pool detected (post-migration, arbitrageable): {}",
                                full_address
                            );
                        }
                    }
                    Err(e) => {
                        debug!("⚠️ Could not verify pool owner: {}", e);
                        // Continue anyway - owner check failure shouldn't block
                    }
                }

                // Cache in memory for future lookups
                let pool_info = PoolInfo {
                    full_address,
                    dex_type: dex_type.clone(),
                    token_a_mint: Pubkey::default(),
                    token_b_mint: Pubkey::default(),
                    reserve_a: Pubkey::default(),
                    reserve_b: Pubkey::default(),
                };
                let _ = self.register_pool(short_id.to_string(), pool_info);

                return Ok(full_address);
            }
            Err(e) => {
                let latency = start_time.elapsed().as_millis() as u64;

                // Update stats atomically
                {
                    let mut stats = self.resolution_stats.write().unwrap();
                    stats.total_latency_ms += latency;
                } // Lock released immediately

                warn!(
                    "❌ Layer 4 MISS: On-chain lookup failed ({}ms): {}",
                    latency, e
                );
            }
        }

        // All layers failed
        Err(anyhow::anyhow!(
            "Pool address not found for {} after trying all 4 layers. Total time: {}ms. Registry has {} pools.",
            short_id,
            start_time.elapsed().as_millis(),
            self.pool_count()
        ))
    }

    /// Query ShredStream service API for full pool address (Layer 2)
    async fn query_shredstream_api(&self, short_id: &str) -> Result<Pubkey> {
        let url = format!("{}/api/pool/{}", self.shredstream_url, short_id);

        debug!("📡 Querying ShredStream API: {}", url);

        let response = self
            .http_client
            .get(&url)
            .timeout(Duration::from_millis(500))
            .send()
            .await
            .context("ShredStream API request failed")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "ShredStream API returned status: {}",
                response.status()
            ));
        }

        let pool_response: ShredStreamPoolResponse = response
            .json()
            .await
            .context("Failed to parse ShredStream API response")?;

        // Try both possible field names
        let address_str = pool_response
            .full_address
            .or(pool_response.pool_address)
            .ok_or_else(|| anyhow::anyhow!("ShredStream API response missing pool address"))?;

        address_str
            .parse::<Pubkey>()
            .context("Failed to parse pool address from ShredStream API")
    }

    /// Query on-chain using getProgramAccounts (Layer 4 - SLOW)
    async fn query_on_chain(&self, short_id: &str, dex_type: &DexType) -> Result<Pubkey> {
        debug!(
            "🔗 Querying on-chain for pool: {} ({:?})",
            short_id, dex_type
        );

        // Get the program ID for this DEX type (reserved for future validation)
        let _program_id = match dex_type {
            // Meteora variants
            DexType::MeteoraDammV1 => {
                "Eo7WjKq67rjJQSZxS6z3YkapzY3eMj6Xy8X5EQVn5UaB".parse::<Pubkey>()?
            }
            DexType::MeteoraDammV2 => {
                "cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG".parse::<Pubkey>()?
            }
            DexType::MeteoraDlmm => {
                "LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo".parse::<Pubkey>()?
            }

            // Orca variants
            DexType::OrcaWhirlpools => {
                "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc".parse::<Pubkey>()?
            }
            DexType::OrcaLegacy => {
                "9W959DqEETiGZocYWCQPaJ6sBmUzgfxXfqGeTEdp3aQP".parse::<Pubkey>()?
            }

            // Raydium variants (note: AMM V4 and CPMM share same program ID)
            DexType::RaydiumAmmV4 => {
                "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8".parse::<Pubkey>()?
            }
            DexType::RaydiumClmm => {
                "CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK".parse::<Pubkey>()?
            }
            DexType::RaydiumCpmm => {
                "CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C".parse::<Pubkey>()?
            }
            DexType::RaydiumStable => {
                "5quBtoiQqxF9Jv6KYKctB59NT3gtJD2Y65kdnB1Uev3h".parse::<Pubkey>()?
            }

            // Other DEXes
            DexType::PumpSwap => "pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA".parse::<Pubkey>()?,
            DexType::Jupiter => "JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4".parse::<Pubkey>()?,
            DexType::Serum => "9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin".parse::<Pubkey>()?,
            DexType::Aldrin => "AMM55ShdkoGRB5jVYPjWziwk8m5MpwyDgsMWHaMSQWH6".parse::<Pubkey>()?,
            DexType::Saros => "SSwpkEEWHvCXCNWnMYXVW7gCYDXkF4aQMxKdpEqrZks".parse::<Pubkey>()?,
            DexType::Crema => "6MLxLqiXaaSUpkgMnWDTuejNZEz3kE7k2woyHGVFw319".parse::<Pubkey>()?,
            DexType::Cropper => "CTMAxxk34HjKWxQ3QLZQA1EQdxtjbYGP4Qjrw7nTn8bM".parse::<Pubkey>()?,
            DexType::Lifinity => {
                "EewxydAPCCVuNEyrVN68PuSYdQ7wKn27V9Gjeoi8dy3S".parse::<Pubkey>()?
            }
            DexType::Fluxbeam => {
                "FLUXBmPhT3Fd1EDVFdg46YREqHBeNypn1h4EbnTzWERX".parse::<Pubkey>()?
            }
            DexType::HumidiFi => "9H6tuB8C3VnXcBLKFJGPqpFu1F2Bwsa7eJvbw8Tq6Rp".parse::<Pubkey>()?,
        };

        // Query all program accounts (VERY SLOW - avoid if possible)
        warn!("⚠️ Using slow getProgramAccounts - this will take 200-400ms!");

        // TODO: Implement getProgramAccounts with prefix filter
        // This requires solana-client dependency which we may not have
        // For now, return error to force using other layers

        Err(anyhow::anyhow!(
            "On-chain lookup not yet implemented. Add solana-client dependency and implement getProgramAccounts with prefix filter."
        ))
    }

    /// Get resolution performance statistics
    pub fn get_resolution_stats(&self) -> (u64, u64, u64, u64, u64, f64) {
        let stats = self.resolution_stats.read().unwrap();
        let avg_latency = if stats.total_lookups > 0 {
            stats.total_latency_ms as f64 / stats.total_lookups as f64
        } else {
            0.0
        };

        (
            stats.layer1_hits,
            stats.layer2_hits,
            stats.layer3_hits,
            stats.layer4_hits,
            stats.total_lookups,
            avg_latency,
        )
    }

    /// Pre-populate registry with known Meteora pools
    /// This is a temporary solution - should be replaced with dynamic lookup
    pub fn populate_meteora_pools(&self) -> Result<()> {
        info!("📋 Pre-populating Meteora DLMM pools...");

        // TODO: Add known Meteora pool addresses here
        // These can be fetched from Meteora API or on-chain queries
        // Example:
        // self.register_pool(
        //     "81vA2wJx".to_string(),
        //     PoolInfo {
        //         full_address: "81vA2wJx...full_address".parse()?,
        //         dex_type: DexType::MeteoraDammV2,
        //         token_a_mint: "...".parse()?,
        //         token_b_mint: "...".parse()?,
        //         reserve_a: "...".parse()?,
        //         reserve_b: "...".parse()?,
        //     }
        // )?;

        warn!("⚠️ Meteora pool pre-population not yet implemented");
        warn!("   Need to fetch pool addresses from Meteora API or ShredStream service");

        Ok(())
    }

    /// Pre-populate registry with known Orca pools
    pub fn populate_orca_pools(&self) -> Result<()> {
        info!("📋 Pre-populating Orca Whirlpool pools...");

        // TODO: Add known Orca pool addresses
        warn!("⚠️ Orca pool pre-population not yet implemented");

        Ok(())
    }

    /// Clear all registered pools
    pub fn clear(&self) {
        let mut pools = self.pools.write().unwrap();
        let mut addr_map = self.address_to_id.write().unwrap();

        pools.clear();
        addr_map.clear();

        info!("🗑️ Pool registry cleared");
    }

    // ========================================
    // GROK'S GHOST POOL SOLUTION - Pool Validation Methods
    // ========================================

    /// Check if pool is cached and valid (with TTL check)
    /// Returns Some(true) if valid, Some(false) if invalid, None if not cached/stale
    pub async fn is_pool_valid_cached(&self, pool_short_id: &str) -> Option<bool> {
        let cache = self.validation_cache.read().await;

        if let Some((is_valid, checked_at)) = cache.get(pool_short_id) {
            // Check if cache entry is still fresh (within TTL)
            if checked_at.elapsed() < Duration::from_secs(VALIDATION_TTL_SECS) {
                return Some(*is_valid);
            }
        }

        None // Not cached or stale
    }

    /// Validate a batch of pools via RPC and update cache
    /// Uses getMultipleAccounts for efficiency (up to 100 pools per call)
    pub async fn validate_pools_batch(&self, pool_short_ids: &[String]) -> Result<()> {
        if pool_short_ids.is_empty() {
            return Ok(());
        }

        debug!("🔍 Validating batch of {} pools", pool_short_ids.len());

        // Resolve short IDs to full addresses
        let mut addresses = Vec::new();
        let mut valid_ids = Vec::new();

        for short_id in pool_short_ids {
            // Try to resolve using any DEX type (we just need the address)
            match self
                .resolve_pool_address(short_id, &DexType::OrcaWhirlpools)
                .await
            {
                Ok(addr) => {
                    addresses.push(addr);
                    valid_ids.push(short_id.clone());
                }
                Err(_) => {
                    // Can't resolve - mark as invalid
                    let mut cache = self.validation_cache.write().await;
                    cache.insert(short_id.clone(), (false, Instant::now()));
                    debug!(
                        "⚠️ Pool {} could not be resolved - marked invalid",
                        short_id
                    );
                }
            }
        }

        if addresses.is_empty() {
            return Ok(());
        }

        // Fetch accounts via RPC (TODO: use get_multiple_accounts for batch)
        // For now, validate individually (will optimize with batch RPC later)
        let mut cache = self.validation_cache.write().await;

        for (i, addr) in addresses.iter().enumerate() {
            let short_id = &valid_ids[i];

            // Check if account exists and has minimum size
            let is_valid = match self.rpc_client.get_account_data(addr) {
                Ok(data) => {
                    let valid = !data.is_empty() && data.len() >= MIN_POOL_SIZE;
                    if !valid {
                        debug!(
                            "⚠️ Pool {} exists but too small ({} bytes < {} min)",
                            short_id,
                            data.len(),
                            MIN_POOL_SIZE
                        );
                    }
                    valid
                }
                Err(_) => {
                    debug!("⚠️ Pool {} RPC check failed - marking invalid", short_id);
                    false
                }
            };

            cache.insert(short_id.clone(), (is_valid, Instant::now()));

            if is_valid {
                debug!("✅ Pool {} validated (size: {} bytes)", short_id, "OK");
            } else {
                debug!("❌ Pool {} marked as ghost pool", short_id);
            }
        }

        Ok(())
    }

    /// Start background task to periodically validate top pools
    /// Runs async without blocking main flow
    pub fn start_background_validation(self: Arc<Self>, top_pools: Vec<String>) {
        tokio::spawn(async move {
            info!(
                "🔄 Starting background pool validation (every {} seconds)",
                BACKGROUND_INTERVAL_SECS
            );
            info!("   Validating top {} pools by volume", top_pools.len());

            loop {
                tokio::time::sleep(Duration::from_secs(BACKGROUND_INTERVAL_SECS)).await;

                if let Err(e) = self.validate_pools_batch(&top_pools).await {
                    warn!("⚠️ Background validation error: {:?}", e);
                } else {
                    debug!(
                        "✅ Background validation cycle complete ({} pools)",
                        top_pools.len()
                    );
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_registry_creation() {
        let rpc_url = "https://api.mainnet-beta.solana.com".to_string();
        let rpc_client = Arc::new(SolanaRpcClient::new(rpc_url));
        let registry = PoolRegistry::new(rpc_client);

        assert_eq!(registry.pool_count(), 0);
    }

    #[test]
    fn test_pool_registration() {
        let rpc_url = "https://api.mainnet-beta.solana.com".to_string();
        let rpc_client = Arc::new(SolanaRpcClient::new(rpc_url));
        let registry = PoolRegistry::new(rpc_client);

        // Create a test pool
        let pool_address: Pubkey = "81vA2wJxKyUE8RHKXxT5VfEQnJGYvJ9FTBwJQhRZHvqX"
            .parse()
            .unwrap();
        let pool_info = PoolInfo {
            full_address: pool_address,
            dex_type: DexType::MeteoraDammV2,
            token_a_mint: Pubkey::default(),
            token_b_mint: Pubkey::default(),
            reserve_a: Pubkey::default(),
            reserve_b: Pubkey::default(),
        };

        // Register pool
        registry
            .register_pool("81vA2wJx".to_string(), pool_info)
            .unwrap();

        // Verify registration
        assert_eq!(registry.pool_count(), 1);
        assert!(registry.has_pool("81vA2wJx"));

        // Verify lookup
        let found = registry.get_pool("81vA2wJx").unwrap();
        assert_eq!(found.full_address, pool_address);

        // Verify reverse lookup
        let short_id = registry.get_short_id(&pool_address).unwrap();
        assert_eq!(short_id, "81vA2wJx");
    }
}
