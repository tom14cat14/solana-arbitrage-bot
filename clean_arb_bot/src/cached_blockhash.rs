// Cached blockhash for fast transaction building
//
// Maintains a fresh blockhash in memory, updated every 400ms by background task.
// This eliminates the 50-70ms RPC latency per transaction build.

use anyhow::Result;
use solana_sdk::hash::Hash;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::rpc_client::SolanaRpcClient;

/// Cached blockhash with timestamp
#[derive(Clone)]
pub struct CachedBlockhash {
    pub hash: Hash,
    pub fetched_at: Instant,
}

/// Shared cached blockhash wrapped in Arc<RwLock> for thread-safe access
pub type SharedCachedBlockhash = Arc<RwLock<Option<CachedBlockhash>>>;

/// Spawn background task to refresh blockhash every 400ms
///
/// Solana blockhashes are valid for ~60 seconds, refreshing every 400ms
/// ensures we always have a fresh one (<1 second old).
///
/// Benefits:
/// - Save 50-70ms per transaction build (no RPC call)
/// - Transactions can be built instantly
/// - Background task handles failures gracefully
pub fn spawn_blockhash_refresher(
    rpc_client: Arc<SolanaRpcClient>,
) -> SharedCachedBlockhash {
    let cached = Arc::new(RwLock::new(None));
    let cached_clone = cached.clone();

    tokio::spawn(async move {
        info!("🔄 Starting blockhash refresh task (every 400ms)");

        let mut consecutive_failures = 0u32;

        loop {
            match rpc_client.get_latest_blockhash() {
                Ok(hash) => {
                    let mut cache = cached_clone.write().await;
                    *cache = Some(CachedBlockhash {
                        hash,
                        fetched_at: Instant::now(),
                    });

                    if consecutive_failures > 0 {
                        info!("✅ Blockhash refresh recovered after {} failures", consecutive_failures);
                        consecutive_failures = 0;
                    } else {
                        debug!("🔄 Blockhash refreshed: {}", hash);
                    }
                }
                Err(e) => {
                    consecutive_failures += 1;
                    if consecutive_failures <= 3 {
                        warn!("⚠️ Failed to refresh blockhash (attempt {}): {}", consecutive_failures, e);
                    } else if consecutive_failures == 10 {
                        warn!("🚨 Blockhash refresh failing ({} consecutive failures) - using stale blockhash", consecutive_failures);
                    }
                    // Keep using old blockhash if fetch fails
                }
            }

            tokio::time::sleep(Duration::from_millis(400)).await;
        }
    });

    info!("✅ Blockhash refresh task started (saves 50-70ms per transaction)");
    cached
}

/// Get cached blockhash, falling back to RPC if not available
///
/// This function prefers the cached blockhash for speed, but will
/// fetch directly from RPC if cache is empty (startup) or very stale (>5s).
pub async fn get_blockhash(
    cached: &SharedCachedBlockhash,
    rpc_client: &SolanaRpcClient,
) -> Result<Hash> {
    // Try cached first
    let cache = cached.read().await;

    if let Some(ref cached_bh) = *cache {
        let age = cached_bh.fetched_at.elapsed();

        // Use cached if < 5 seconds old
        if age < Duration::from_secs(5) {
            debug!("⚡ Using cached blockhash (age: {}ms)", age.as_millis());
            return Ok(cached_bh.hash);
        } else {
            warn!("⚠️ Cached blockhash is stale (age: {}s) - fetching new one", age.as_secs());
        }
    }

    // Cache miss or stale - fetch from RPC
    drop(cache); // Release read lock before fetching

    debug!("🔄 Cache miss - fetching blockhash from RPC");
    let hash = rpc_client.get_latest_blockhash()?;

    // Update cache
    let mut cache = cached.write().await;
    *cache = Some(CachedBlockhash {
        hash,
        fetched_at: Instant::now(),
    });

    Ok(hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cached_blockhash_struct() {
        let hash = Hash::default();
        let cached = CachedBlockhash {
            hash,
            fetched_at: Instant::now(),
        };

        assert_eq!(cached.hash, hash);
        assert!(cached.fetched_at.elapsed() < Duration::from_millis(10));
    }
}
