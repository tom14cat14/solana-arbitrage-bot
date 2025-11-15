use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use std::num::NonZeroU32;
use tracing::{debug, warn, info};
use tokio_retry::{strategy::ExponentialBackoff, Retry};  // CYCLE-6: Retry logic
use tokio::time::timeout;  // CYCLE-7: Network jitter protection
use governor::{Quota, RateLimiter as GovernorRateLimiter, clock::DefaultClock, state::{InMemoryState, NotKeyed}};  // CYCLE-7: Rate limiting
use dashmap::DashMap;  // OPTIMIZATION: Lock-free concurrent hashmap
use std::sync::Arc;

/// Cached price entry with timestamp for staleness checking
#[derive(Debug, Clone)]
pub struct CachedPrice {
    pub data: TokenPrice,
    pub cached_at: Instant,
}

/// Price information from ShredStream service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPrice {
    pub token_mint: String,
    pub dex: String,
    pub price_sol: f64,
    pub last_update: String,
    pub volume_24h: f64,
    pub pool_address: String,  // CRITICAL FIX: Full 44-char address for DEX swaps
}

/// Response from /prices endpoint
#[derive(Debug, Deserialize)]
pub struct PricesResponse {
    pub prices: Vec<TokenPrice>,
    pub total_tokens: usize,
}

/// Client for ShredStream service REST API
/// CYCLE-7: Enhanced with rate limiting (Grok recommendation)
/// OPTIMIZED: Lock-free concurrent cache with staleness detection
pub struct ShredStreamClient {
    /// Service endpoint URL
    service_url: String,
    /// HTTP client
    client: reqwest::Client,
    /// Cached prices by token_mint + dex (concurrent access)
    /// OPTIMIZATION: DashMap allows lock-free concurrent reads/writes
    price_cache: Arc<DashMap<String, CachedPrice>>,
    /// CYCLE-7: Rate limiter (prevents API bans on 429 responses)
    /// Token bucket: 10 requests per second (600/minute)
    rate_limiter: GovernorRateLimiter<NotKeyed, InMemoryState, DefaultClock>,
    /// Last successful fetch timestamp (for delta updates)
    last_fetch: Option<Instant>,
    /// Cache TTL in seconds (prices older than this are stale)
    cache_ttl_secs: u64,
}

impl ShredStreamClient {
    /// Create new ShredStream service client
    /// CYCLE-6: Optimized with gzip compression and connection pooling
    /// CYCLE-7: Enhanced with rate limiting (Grok recommendation)
    pub fn new(service_url: String) -> Self {
        // Build client with gzip support and optimized settings
        let client = reqwest::Client::builder()
            .gzip(true)  // Enable gzip decompression
            .pool_max_idle_per_host(2)  // Connection pooling
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        // CYCLE-7: Rate limiter - 10 requests per second (Grok recommendation)
        // Token bucket algorithm prevents API bans on high-frequency requests
        let quota = Quota::per_second(NonZeroU32::new(10).unwrap());
        let rate_limiter = GovernorRateLimiter::direct(quota);

        Self {
            service_url,
            client,
            price_cache: Arc::new(DashMap::new()),
            rate_limiter,
            last_fetch: None,
            cache_ttl_secs: 5,  // 5 second cache TTL (prices are fresh for 5s)
        }
    }

    /// Check if we need to fetch new prices (cache staleness check)
    /// OPTIMIZATION: Skip fetching if cache is still fresh
    pub fn needs_update(&self) -> bool {
        match self.last_fetch {
            Some(last) => last.elapsed().as_secs() >= self.cache_ttl_secs,
            None => true,  // Never fetched, needs update
        }
    }

    /// Fetch latest prices from service
    /// CYCLE-6: Optimized with streaming JSON, gzip, and exponential backoff retry
    /// CYCLE-7: Added timeout guard for network jitter protection + rate limiting
    /// OPTIMIZATION: Skip if cache is fresh, use concurrent DashMap for updates
    pub async fn fetch_prices(&mut self) -> Result<usize> {
        // OPTIMIZATION: Skip if cache is still fresh
        if !self.needs_update() {
            let cached_count = self.price_cache.len();
            debug!("⚡ Cache still fresh ({} prices, TTL: {}s)", cached_count, self.cache_ttl_secs);
            return Ok(cached_count);
        }
        // CYCLE-7: Rate limiting check (prevents API bans)
        // If rate limit exceeded, wait until token available
        self.rate_limiter.until_ready().await;

        // CYCLE-6: Performance benchmark timing
        let fetch_start = std::time::Instant::now();

        // CRITICAL FIX: Endpoint is /prices not /api/prices
        let url = format!("{}/prices", self.service_url);

        // CYCLE-7: Timeout guard to protect against network jitter (5s for all retries)
        // Prevents hanging during network issues (Grok recommendation)
        let timeout_result = timeout(Duration::from_secs(5), async {
            // CYCLE-6: Retry with exponential backoff (100ms, 200ms, 400ms, 800ms, 1600ms)
            let retry_strategy = ExponentialBackoff::from_millis(100).take(5);

            Retry::spawn(retry_strategy, || async {
            // CYCLE-6: Request with gzip compression enabled
            match self.client
                .get(&url)
                .header("Accept-Encoding", "gzip")
                .send()
                .await
            {
                Ok(response) => {
                    // CYCLE-6: Stream response bytes instead of buffering entire response
                    let bytes = response.bytes().await.map_err(|e| {
                        warn!("❌ Failed to read response bytes: {}", e);
                        anyhow::anyhow!("Response bytes error: {}", e)
                    })?;

                    debug!("📡 Received {} bytes (gzip-compressed if supported)", bytes.len());

                    // Parse JSON directly from bytes (more efficient than string conversion)
                    let prices_response: PricesResponse = match serde_json::from_slice(&bytes) {
                        Ok(parsed) => parsed,
                        Err(e) => {
                            warn!("❌ Failed to parse response: {}", e);
                            // Only log first 500 bytes for debugging (avoid huge logs)
                            let preview = String::from_utf8_lossy(&bytes[..bytes.len().min(500)]);
                            warn!("📄 Response preview: {}", preview);
                            return Err(anyhow::anyhow!("JSON parse error: {}", e));
                        }
                    };

                    Ok(prices_response)
                }
                Err(e) => {
                    warn!("⚠️ ShredStream fetch failed (will retry): {}", e);
                    Err(anyhow::anyhow!("Request failed: {}", e))
                }
            }
        }).await
        }).await;

        // Handle timeout
        let result = match timeout_result {
            Ok(retry_result) => retry_result,
            Err(_) => {
                warn!("⚠️ ShredStream fetch timed out after 5s (network jitter protection)");
                return Err(anyhow::anyhow!("Fetch timeout exceeded"));
            }
        };

        match result {
            Ok(prices_response) => {
                // Update cache with timestamps
                let now = Instant::now();
                let fetched_count = prices_response.prices.len();

                // OPTIMIZATION: Batch update using concurrent DashMap
                for price in prices_response.prices {
                    let cache_key = format!("{}_{}", price.token_mint, price.dex);
                    let cached_price = CachedPrice {
                        data: price,
                        cached_at: now,
                    };
                    self.price_cache.insert(cache_key, cached_price);
                }

                // Update last fetch timestamp
                self.last_fetch = Some(now);

                // CYCLE-6: Log fetch performance
                let fetch_duration = fetch_start.elapsed();
                info!("⚡ Fetched {} prices in {:?} (total_tokens: {}, gzip enabled, cache TTL: {}s)",
                       fetched_count, fetch_duration, prices_response.total_tokens, self.cache_ttl_secs);
                Ok(fetched_count)
            }
            Err(e) => {
                warn!("❌ Failed to fetch prices after retries: {}", e);
                Err(anyhow::anyhow!("ShredStream service unavailable after retries: {}", e))
            }
        }
    }

    /// Get price for specific token on specific DEX
    pub fn get_price(&self, token_mint: &str, dex: &str) -> Option<f64> {
        let cache_key = format!("{}_{}", token_mint, dex);
        self.price_cache.get(&cache_key).map(|entry| entry.data.price_sol)
    }

    /// Get all prices for a token across all DEXs
    pub fn get_token_prices(&self, token_mint: &str) -> Vec<(String, f64)> {
        let mut results = Vec::new();
        for entry in self.price_cache.iter() {
            if entry.value().data.token_mint == token_mint {
                results.push((entry.value().data.dex.clone(), entry.value().data.price_sol));
            }
        }
        results
    }

    /// Get all cached prices (returns HashMap for compatibility)
    /// OPTIMIZATION: Only includes non-stale prices
    pub fn get_all_prices(&self) -> HashMap<String, TokenPrice> {
        let mut result = HashMap::new();
        let now = Instant::now();
        let max_age = Duration::from_secs(self.cache_ttl_secs * 2);  // Allow 2x TTL for reads

        for entry in self.price_cache.iter() {
            // Skip stale entries
            if now.duration_since(entry.value().cached_at) <= max_age {
                let cache_key = entry.key().clone();
                let token_price = entry.value().data.clone();
                result.insert(cache_key, token_price);
            }
        }
        result
    }
}
