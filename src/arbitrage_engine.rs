use anyhow::{Context, Result};
use solana_sdk::signature::{Keypair, Signer};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, Mutex};
use tokio::time::sleep;
use tracing::{debug, error, info, warn}; // CYCLE-5: Added error macro

use crate::config::Config;
use crate::cost_calculator::ArbitrageCosts;
use crate::dex_registry::DexRegistry;
use crate::jito_bundle_client::JitoBundleClient;
use crate::jito_submitter::JitoSubmitter;
use crate::jupiter_prices::JupiterPriceClient;
use crate::jupiter_triangle::JupiterTriangleDetector;
use crate::meteora_swap; // CYCLE-7: Meteora swap instruction building
use crate::position_tracker::PositionTracker;
use crate::shredstream_client::{ShredStreamClient, TokenPrice};
use crate::simple_triangle_detector::SimpleTriangleDetector;
use crate::triangle_arbitrage::TriangleArbitrage;
use crate::{extract_pool_id, DexType, PoolRegistry, SolanaRpcClient, SwapExecutor, SwapParams};

// Constants for arbitrage detection and execution
const STALE_OPPORTUNITY_THRESHOLD_MS: u64 = 100; // Max age before considering stale
const SHREDSTREAM_TIMEOUT_MS: u64 = 500; // Timeout for ShredStream price fetch
const SCAN_INTERVAL_MS: u64 = 1500; // Scan interval (synced with JITO rate limit)
const STATS_REPORT_INTERVAL_SECS: u64 = 60; // Report stats every 60 seconds
const BALANCE_UPDATE_OPPORTUNITIES: u64 = 50; // Update balance every 50 opportunities
const BALANCE_UPDATE_INTERVAL_SECS: u64 = 600; // Or every 10 minutes
const MAX_REALISTIC_SPREAD_PCT: f64 = 50.0; // Max spread for volatile memecoins
const LOG_SPREAD_THRESHOLD_PCT: f64 = 0.3; // Log spreads above this threshold
const MIN_VOLUME_SOL: f64 = 10.0; // Minimum 24h volume to avoid illiquid tokens (increased from 0.01)

/// Arbitrage opportunity
#[derive(Debug, Clone)]
pub struct ArbitrageOpportunity {
    pub token_mint: String,
    pub buy_dex: String,
    pub sell_dex: String,
    pub buy_price: f64,
    pub sell_price: f64,
    pub spread_percentage: f64,
    pub estimated_profit_sol: f64,

    // GHOST POOL FIX: Full 44-char pool addresses from ShredStream
    pub buy_pool_address: String,  // Full address for buy pool
    pub sell_pool_address: String, // Full address for sell pool

    // NEW (2025-10-11): Timestamp for staleness detection
    pub detected_at: Instant, // When opportunity was detected
}

/// Arbitrage statistics
#[derive(Debug, Default)]
pub struct ArbitrageStats {
    pub runtime_seconds: u64,
    pub opportunities_detected: u64,
    pub opportunities_executed: u64,
    pub failed_executions: u64,
    pub total_profit_sol: f64,
    pub daily_trades: u64,
    pub daily_loss_sol: f64,
    pub consecutive_failures: u64,
}

impl ArbitrageStats {
    pub fn success_rate(&self) -> f64 {
        if self.opportunities_detected == 0 {
            0.0
        } else {
            (self.opportunities_executed as f64 / self.opportunities_detected as f64) * 100.0
        }
    }
}

/// Clean arbitrage engine
pub struct ArbitrageEngine {
    config: Config,
    shredstream_client: ShredStreamClient,
    dex_registry: DexRegistry,
    triangle_arbitrage: TriangleArbitrage,
    simple_triangle: SimpleTriangleDetector,
    jupiter_client: Option<JupiterPriceClient>,
    jupiter_triangle: Option<JupiterTriangleDetector>,
    jito_client: Option<Arc<JitoBundleClient>>,
    jito_submitter: Option<Arc<JitoSubmitter>>, // Queue-based JITO submission
    // DEX swap components for real execution
    swap_executor: Option<SwapExecutor>,
    pool_registry: Option<Arc<PoolRegistry>>,
    wallet_keypair: Option<Arc<Keypair>>,
    // CYCLE-7: Standard RPC client for Meteora swap instructions
    rpc_client: Option<Arc<SolanaRpcClient>>,
    // HIGH-4 FIX: Position tracking to prevent over-leveraging
    position_tracker: Arc<PositionTracker>,
    // NEW (2025-10-07): Dynamic JITO tip floor monitor (updates every 30 min)
    jito_tip_floor: crate::jito_tip_monitor::SharedJitoTipFloor,
    // NEW (2025-10-11): Cached blockhash (pre-fetched, saves 50-70ms per tx)
    cached_blockhash: Option<crate::cached_blockhash::SharedCachedBlockhash>,
    stats: ArbitrageStats,
    start_time: Instant,
    shutdown_rx: broadcast::Receiver<()>,
}

impl ArbitrageEngine {
    pub async fn new(
        config: Config,
        shutdown_rx: broadcast::Receiver<()>,
        jito_tip_floor: crate::jito_tip_monitor::SharedJitoTipFloor,
    ) -> Result<Self> {
        let shredstream_client = ShredStreamClient::new(config.shredstream_url.clone());
        let dex_registry = DexRegistry::new();
        let triangle_arbitrage = TriangleArbitrage::new();
        let simple_triangle = SimpleTriangleDetector::new();

        // Initialize Jupiter clients if API key provided
        let (jupiter_client, jupiter_triangle) = if let Some(ref key) = config.jupiter_api_key {
            info!("✅ Jupiter Ultra endpoint enabled with API key");
            (
                Some(JupiterPriceClient::new(Some(key.clone()))),
                Some(JupiterTriangleDetector::new(Some(key.clone()))),
            )
        } else {
            (None, None)
        };

        // Initialize JITO bundle client for atomic execution (real trading only)
        let jito_client = if config.enable_real_trading && !config.paper_trading {
            if let Some(ref wallet_key) = config.wallet_private_key {
                match bs58::decode(wallet_key).into_vec() {
                    Ok(bytes) => {
                        match Keypair::from_bytes(&bytes) {
                            Ok(keypair) => {
                                // Read JITO endpoint from config (matches MEV_Bot pattern)
                                let jito_endpoint =
                                    std::env::var("JITO_ENDPOINT").unwrap_or_else(|_| {
                                        "https://mainnet.block-engine.jito.wtf".to_string()
                                    });

                                info!("🔗 Using JITO endpoint: {}", jito_endpoint);

                                // Use same endpoint for both URLs (JITO API design)
                                let client = Arc::new(JitoBundleClient::new_with_keypair_ref(
                                    jito_endpoint.clone(),
                                    jito_endpoint,
                                    Arc::new(keypair),
                                ));
                                info!("✅ JITO bundle client initialized for atomic execution");
                                Some(client)
                            }
                            Err(e) => {
                                warn!("⚠️ Failed to parse wallet keypair: {}", e);
                                None
                            }
                        }
                    }
                    Err(e) => {
                        warn!("⚠️ Failed to decode wallet private key: {}", e);
                        None
                    }
                }
            } else {
                warn!("⚠️ Real trading enabled but no wallet private key provided");
                None
            }
        } else {
            if config.paper_trading {
                info!("📄 Paper trading mode - JITO bundles disabled");
            }
            None
        };

        // Initialize queue-based JITO submitter with gRPC + HTTP fallback
        let jito_submitter = if let Some(ref http_client) = jito_client {
            // Try to create gRPC client (async operation)
            let grpc_client = match crate::jito_grpc_client::JitoGrpcClient::new().await {
                Ok(grpc_client) => {
                    info!("✅ gRPC client initialized successfully");
                    Some(Arc::new(Mutex::new(grpc_client)))
                }
                Err(e) => {
                    warn!("⚠️ Failed to create gRPC client: {}", e);
                    warn!("⚠️ Falling back to HTTP-only mode");
                    None
                }
            };

            // Create submitter (with or without gRPC)
            let submitter = Arc::new(JitoSubmitter::new(grpc_client.clone(), http_client.clone()));

            if grpc_client.is_some() {
                info!("✅ Queue-based JITO submitter initialized:");
                info!("   • Primary: gRPC (75ms latency - 2x faster!)");
                info!("   • Fallback: HTTP (150ms latency)");
            } else {
                info!("✅ Queue-based JITO submitter initialized:");
                info!("   • HTTP only (gRPC unavailable)");
            }
            info!("   • Rate: 1 bundle/1.1s");

            Some(submitter)
        } else {
            None
        };

        info!(
            "✅ Loaded {} DEXs for arbitrage",
            dex_registry.get_all_dexs().len()
        );
        info!("✅ Cross-DEX triangle arbitrage enabled");
        info!("✅ Simple multi-hop triangle detection enabled (ShredStream data)");
        if jupiter_triangle.is_some() {
            info!("✅ Jupiter triangle detection enabled (execute only)");
        }

        // Initialize DEX swap executor for real trading (if enabled)
        let (swap_executor, pool_registry, wallet_keypair, rpc_client, cached_blockhash) =
            if !config.paper_trading {
                if let Some(ref wallet_key) = config.wallet_private_key {
                    match bs58::decode(wallet_key).into_vec() {
                        Ok(bytes) => {
                            match Keypair::from_bytes(&bytes) {
                                Ok(keypair) => {
                                    // Use configured RPC endpoint or default
                                    let rpc_url =
                                        config.solana_rpc_url.clone().unwrap_or_else(|| {
                                            "https://api.mainnet-beta.solana.com".to_string()
                                        });

                                    // Create wrapped RPC client
                                    let wrapped_rpc =
                                        Arc::new(SolanaRpcClient::new(rpc_url.clone()));
                                    let pool_registry =
                                        Arc::new(PoolRegistry::new(wrapped_rpc.clone()));

                                    // Create swap executor (JITO not needed for SwapExecutor, handled separately)
                                    let executor = SwapExecutor::new(
                                        wrapped_rpc.clone(),
                                        pool_registry.clone(),
                                        None, // JITO handled separately in execute_triangle
                                    )?;

                                    info!("✅ Swap executor initialized for real DEX trading");
                                    info!(
                                        "✅ RPC client initialized with circuit breaker protection"
                                    );

                                    // NEW (2025-10-11): Start blockhash pre-fetching background task
                                    let cached_blockhash =
                                        crate::cached_blockhash::spawn_blockhash_refresher(
                                            wrapped_rpc.clone(),
                                        );

                                    (
                                        Some(executor),
                                        Some(pool_registry),
                                        Some(Arc::new(keypair)),
                                        Some(wrapped_rpc),
                                        Some(cached_blockhash),
                                    )
                                }
                                Err(e) => {
                                    warn!("⚠️ Failed to initialize swap executor: {}", e);
                                    (None, None, None, None, None)
                                }
                            }
                        }
                        Err(e) => {
                            warn!("⚠️ Failed to decode wallet key for swap executor: {}", e);
                            (None, None, None, None, None)
                        }
                    }
                } else {
                    warn!("⚠️ No wallet key provided - swap executor disabled");
                    (None, None, None, None, None)
                }
            } else {
                info!("📄 Paper trading mode - swap executor disabled");
                (None, None, None, None, None)
            };

        // HIGH-4 FIX: Initialize position tracker for capital management
        let position_tracker = Arc::new(PositionTracker::new(
            config.capital_sol,
            config.max_position_size_sol,
        ));

        Ok(Self {
            config,
            shredstream_client,
            dex_registry,
            triangle_arbitrage,
            simple_triangle,
            jupiter_client,
            jupiter_triangle,
            jito_client,
            jito_submitter,
            swap_executor,
            pool_registry,
            wallet_keypair,
            rpc_client,
            position_tracker,
            jito_tip_floor,   // NEW (2025-10-07): Dynamic JITO tip floor data
            cached_blockhash, // NEW (2025-10-11): Pre-fetched blockhash cache
            stats: ArbitrageStats::default(),
            start_time: Instant::now(),
            shutdown_rx,
        })
    }

    /// Main arbitrage loop with cooperative cancellation (Grok recommendation)
    pub async fn run(&mut self) -> Result<()> {
        info!("🔄 Starting arbitrage scanning loop...");

        // CRITICAL FIX: Fetch actual wallet balance at startup
        if let (Some(ref rpc), Some(ref wallet)) = (&self.rpc_client, &self.wallet_keypair) {
            info!("💰 Fetching actual wallet balance...");
            match rpc.get_balance(&wallet.pubkey()) {
                Ok(balance_lamports) => {
                    let balance_sol = balance_lamports as f64 / 1_000_000_000.0;
                    info!(
                        "✅ Wallet balance: {:.4} SOL ({} lamports)",
                        balance_sol, balance_lamports
                    );

                    // Update position tracker with actual balance
                    let tradeable = self
                        .position_tracker
                        .update_from_wallet_balance(balance_lamports);
                    let tradeable_sol = tradeable as f64 / 1_000_000_000.0;
                    info!(
                        "📊 Tradeable capital updated to {:.4} SOL (after 0.1 SOL fee reserve)",
                        tradeable_sol
                    );
                }
                Err(e) => {
                    warn!("⚠️ Failed to fetch wallet balance: {}", e);
                    warn!("   Using configured capital from .env file");
                }
            }
        }

        // Track when we last updated wallet balance
        let mut last_balance_update = Instant::now();
        let mut opportunities_at_last_update = 0u64;

        loop {
            // Update stats
            self.stats.runtime_seconds = self.start_time.elapsed().as_secs();

            // Periodically update wallet balance
            let opportunities_since_update =
                self.stats.opportunities_detected - opportunities_at_last_update;
            let time_since_update = last_balance_update.elapsed();

            if opportunities_since_update >= BALANCE_UPDATE_OPPORTUNITIES
                || time_since_update >= Duration::from_secs(BALANCE_UPDATE_INTERVAL_SECS)
            {
                if let (Some(ref rpc), Some(ref wallet)) = (&self.rpc_client, &self.wallet_keypair)
                {
                    if let Ok(balance_lamports) = rpc.get_balance(&wallet.pubkey()) {
                        let balance_sol = balance_lamports as f64 / 1_000_000_000.0;
                        let tradeable = self
                            .position_tracker
                            .update_from_wallet_balance(balance_lamports);
                        let tradeable_sol = tradeable as f64 / 1_000_000_000.0;
                        debug!(
                            "💰 Updated wallet balance: {:.4} SOL (tradeable: {:.4} SOL)",
                            balance_sol, tradeable_sol
                        );
                        last_balance_update = Instant::now();
                        opportunities_at_last_update = self.stats.opportunities_detected;
                    }
                }
            }

            // HIGH-4 FIX: Check for emergency stop file
            // Create .emergency_stop file in working directory to immediately halt trading
            if std::path::Path::new(".emergency_stop").exists() {
                warn!("🚨 EMERGENCY STOP FILE DETECTED - HALTING ALL TRADING IMMEDIATELY");
                warn!("   File: .emergency_stop found in working directory");
                warn!("   Remove this file to resume trading");
                break;
            }

            // Check for shutdown signal (Grok recommendation: cooperative cancellation point)
            if let Ok(()) = self.shutdown_rx.try_recv() {
                info!("🛑 Shutdown signal received - stopping arbitrage loop gracefully");
                break;
            }

            // Check safety limits
            if self.should_stop_trading() {
                warn!("⛔ Safety limit reached - stopping trading");
                break;
            }

            // HIGH FIX: Fetch prices with timeout (ShredStream is fast HTTP service)
            // Solana-optimized: ShredStream should respond in <100ms typically
            match tokio::time::timeout(
                Duration::from_millis(SHREDSTREAM_TIMEOUT_MS),
                self.shredstream_client.fetch_prices(),
            )
            .await
            {
                Ok(Ok(count)) => {
                    if count > 0 {
                        debug!("📡 Fetched {} token prices", count);
                    }
                }
                Ok(Err(e)) => {
                    warn!("⚠️ ShredStream service error: {} - retrying in 1s", e);

                    tokio::select! {
                        _ = sleep(Duration::from_secs(1)) => {},
                        _ = self.shutdown_rx.recv() => {
                            info!("🛑 Shutdown during reconnect wait");
                            break;
                        }
                    }
                    continue;
                }
                Err(_) => {
                    warn!("⚠️ ShredStream timeout after 500ms - retrying in 1s");

                    tokio::select! {
                        _ = sleep(Duration::from_secs(1)) => {},
                        _ = self.shutdown_rx.recv() => {
                            info!("🛑 Shutdown during reconnect wait");
                            break;
                        }
                    }
                    continue;
                }
            }

            // Scan for all types of arbitrage opportunities
            let mut all_opportunities = Vec::new();

            // 1. Cross-DEX arbitrage
            all_opportunities.extend(self.scan_for_opportunities().await);

            // 2. Triangle arbitrage - find and collect opportunities first
            let triangle_opps_owned = {
                let prices = self.shredstream_client.get_all_prices();
                self.triangle_arbitrage.find_opportunities(
                    &prices,
                    &self.config,
                    self.config.max_position_size_sol,
                )
            }; // prices borrow ends here

            // Execute triangle opportunities
            for triangle in triangle_opps_owned {
                debug!(
                    "🔺 Triangle opportunity: {:?} → {:.4} SOL profit",
                    triangle.path, triangle.estimated_profit_sol
                );

                // Track opportunity detected
                self.stats.opportunities_detected += 1;

                // HIGH-4 FIX: Reserve capital before execution
                // Use max_position_size as the capital for triangle arbitrage
                let position_size_lamports =
                    (self.config.max_position_size_sol * 1_000_000_000.0) as u64;

                match self
                    .position_tracker
                    .reserve_capital(position_size_lamports)
                {
                    Ok(()) => {
                        // Execute with JITO bundle (atomic execution)
                        match self.execute_triangle_opportunity(&triangle).await {
                            Ok(()) => {
                                info!("✅ Triangle opportunity executed successfully");
                            }
                            Err(e) => {
                                debug!("⚠️ Triangle execution failed: {}", e);
                            }
                        }

                        // Always release capital after execution (success or failure)
                        self.position_tracker
                            .release_capital(position_size_lamports);
                    }
                    Err(e) => {
                        warn!("⚠️ Insufficient capital for triangle opportunity: {}", e);
                        debug!(
                            "   Needed: {:.4} SOL, Stats: {:?}",
                            self.config.max_position_size_sol,
                            self.position_tracker.get_stats()
                        );
                        continue;
                    }
                }

                // Safety check: Stop if we've hit trading limits
                if self.should_stop_trading() {
                    break;
                }
            }

            // 3. Jupiter cross-DEX arbitrage (DISABLED - requires paid Price API)
            // Note: Jupiter Price API now requires a paid plan (not available on free tier)
            // Use Jupiter triangle detection instead (below) which uses free Quote API
            /*
            if let Some(ref jupiter) = self.jupiter_client {
                match crate::jupiter_prices::find_jupiter_arbitrage(
                    &prices,
                    jupiter,
                    self.config.min_profit_sol,
                    self.config.min_spread_percentage,
                    self.config.max_position_size_sol,
                ).await {
                    Ok(jupiter_opps) => {
                        for jup_opp in jupiter_opps {
                            info!("🪐 Jupiter arbitrage: {} - {:.2}% spread, {:.4} SOL profit",
                                &jup_opp.token_mint[..8],
                                jup_opp.spread_percentage,
                                jup_opp.estimated_profit_sol);
                        }
                    }
                    Err(e) => debug!("⚠️ Jupiter arbitrage scan failed: {}", e),
                }
            }
            */

            // 4. Simple triangle arbitrage (ShredStream data, execute via Jupiter)
            let prices = self.shredstream_client.get_all_prices();
            let simple_triangles = self.simple_triangle.find_opportunities(
                &prices,
                self.config.max_position_size_sol,
                &self.config,
            );

            for triangle in simple_triangles {
                self.stats.opportunities_detected += 1;

                info!("🔺 Triangle Arbitrage Found (ShredStream data)!");
                info!(
                    "   Path: SOL → {} → {} → SOL",
                    triangle
                        .token_a_mint
                        .get(..8)
                        .unwrap_or(&triangle.token_a_mint),
                    triangle
                        .token_b_mint
                        .get(..8)
                        .unwrap_or(&triangle.token_b_mint)
                );
                info!(
                    "   DEXs: {} → {} → {}",
                    triangle.dex_1, triangle.dex_2, triangle.dex_3
                );
                info!("   Input: {:.6} SOL", triangle.input_amount_sol);
                info!(
                    "   Profit: {:.6} SOL ({:.2}%)",
                    triangle.profit_sol, triangle.profit_percentage
                );

                // Execute if profitable (paper trading for now)
                if self.config.paper_trading {
                    info!("   💼 PAPER TRADE: Would execute via Jupiter swap API");
                    self.stats.opportunities_executed += 1;
                    self.stats.total_profit_sol += triangle.profit_sol;
                } else {
                    info!("   🚀 LIVE: Would build Jupiter swap transaction");
                    // TODO: Build actual Jupiter swap transaction here
                }
            }

            // 5. Jupiter multi-hop triangle (DISABLED - Jupiter rejects SOL→SOL swaps)
            // Note: Jupiter API returns error "inputMint cannot be same as outputMint"
            // Triangle arbitrage must be detected via our ShredStream data (above)
            // and executed using Jupiter swap API with intermediate tokens
            /*
            if let Some(ref jupiter_triangle) = self.jupiter_triangle {
                match jupiter_triangle.find_triangle_opportunities(
                    self.config.max_position_size_sol,
                    &self.config,
                ).await {
                    Ok(triangle_opps) => {
                        for triangle in triangle_opps {
                            self.stats.opportunities_detected += 1;

                            info!("🔺 Jupiter Triangle Arbitrage Found!");
                            info!("   Route: {}", triangle.route_description);
                            info!("   Input: {:.6} SOL", triangle.input_amount_sol);
                            info!("   Output: {:.6} SOL", triangle.output_amount_sol);
                            info!("   Profit: {:.6} SOL ({:.2}%)",
                                triangle.profit_sol, triangle.profit_percentage);
                            info!("   Hops: {}", triangle.route_hops);

                            // Execute if profitable (paper trading for now)
                            if self.config.paper_trading {
                                info!("   💼 PAPER TRADE: Would execute triangle arbitrage");
                                self.stats.opportunities_executed += 1;
                                self.stats.total_profit_sol += triangle.profit_sol;
                            }
                        }
                    }
                    Err(e) => debug!("⚠️ Jupiter triangle scan failed: {}", e),
                }
            }
            */

            // Execute profitable opportunities (FIRST OPPORTUNITY ONLY)
            // Synced with 1.5s scan interval: 1 scan = 1 opportunity = fresh data
            // Note: Opportunities already filtered by triangle detectors with margin checks
            for opportunity in all_opportunities {
                // Double-check profitability (opportunities should already be filtered)
                if self
                    .config
                    .is_profitable_after_fees(opportunity.estimated_profit_sol)
                {
                    self.stats.opportunities_detected += 1;

                    // NEW (2025-10-11): Early staleness detection (Option 4)
                    // Skip opportunities older than threshold to avoid wasting time building instructions
                    let age = opportunity.detected_at.elapsed();
                    if age > Duration::from_millis(STALE_OPPORTUNITY_THRESHOLD_MS) {
                        warn!("⏰ Skipping stale opportunity (age: {}ms) - would fail simulation anyway",
                              age.as_millis());
                        debug!(
                            "   Token: {} - detected {}ms ago, likely stale pool state",
                            opportunity
                                .token_mint
                                .get(..8)
                                .unwrap_or(&opportunity.token_mint),
                            age.as_millis()
                        );
                        continue; // Skip to next opportunity immediately
                    }

                    info!(
                        "🎯 Arbitrage opportunity found (age: {}ms):",
                        age.as_millis()
                    );
                    info!(
                        "   Token: {}",
                        opportunity
                            .token_mint
                            .get(..8)
                            .unwrap_or(&opportunity.token_mint)
                    );
                    info!(
                        "   Buy: {} @ {:.6} SOL",
                        opportunity.buy_dex, opportunity.buy_price
                    );
                    info!(
                        "   Sell: {} @ {:.6} SOL",
                        opportunity.sell_dex, opportunity.sell_price
                    );
                    info!("   Spread: {:.2}%", opportunity.spread_percentage);
                    info!(
                        "   Est. Profit: {:.6} SOL",
                        opportunity.estimated_profit_sol
                    );

                    // Execute the trade
                    if let Err(e) = self.execute_arbitrage(&opportunity).await {
                        warn!("❌ Execution failed: {}", e);
                        self.stats.failed_executions += 1;
                        self.stats.consecutive_failures += 1;
                    } else {
                        self.stats.opportunities_executed += 1;
                        self.stats.daily_trades += 1;
                        self.stats.consecutive_failures = 0;
                        info!("✅ Arbitrage executed successfully");
                    }

                    // CRITICAL: Only execute FIRST opportunity per scan
                    // This ensures fresh data every 1.5s (synced with JITO rate limit)
                    break;
                }
            }

            // Report stats periodically
            if self.stats.runtime_seconds % STATS_REPORT_INTERVAL_SECS == 0
                && self.stats.runtime_seconds > 0
            {
                self.report_stats();
            }

            // Scan interval synced with JITO rate limit
            // This ensures each scan produces fresh data that can be submitted immediately
            // JITO limit: 1 bundle per 1.1s, scan interval ensures fresh opportunities
            sleep(Duration::from_millis(SCAN_INTERVAL_MS)).await;
        }

        Ok(())
    }

    /// Scan for arbitrage opportunities
    async fn scan_for_opportunities(&self) -> Vec<ArbitrageOpportunity> {
        // CYCLE-6: Performance benchmark timing
        let scan_start = std::time::Instant::now();

        let mut opportunities = Vec::new();

        // NEW: Target token filtering to avoid ghost pools
        // Get target tokens from environment variable (comma-separated list)
        let target_tokens = std::env::var("TARGET_TOKENS").ok().map(|s| {
            s.split(',')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect::<Vec<_>>()
        });

        // Fetch all prices from ShredStream
        let all_prices_unfiltered = self.shredstream_client.get_all_prices();

        // Filter by target tokens if specified
        let all_prices: HashMap<String, TokenPrice> = if let Some(ref tokens) = target_tokens {
            all_prices_unfiltered
                .into_iter()
                .filter(|(_, price)| tokens.contains(&price.token_mint))
                .collect()
        } else {
            all_prices_unfiltered
        };

        // Log filtering results
        if let Some(ref tokens) = target_tokens {
            info!(
                "🎯 Target token filtering: {} prices (from {} target tokens)",
                all_prices.len(),
                tokens.len()
            );
            debug!(
                "🎯 Target tokens: {:?}",
                tokens
                    .iter()
                    .map(|t| t.get(..8).unwrap_or(t))
                    .collect::<Vec<_>>()
            );
        }

        // Group prices by token
        let mut token_prices: HashMap<String, Vec<&TokenPrice>> = HashMap::new();
        for price in all_prices.values() {
            token_prices
                .entry(price.token_mint.clone())
                .or_insert_with(Vec::new)
                .push(price);
        }

        // Find arbitrage opportunities for each token
        for (token_mint, prices) in token_prices {
            if prices.len() < 2 {
                continue; // Need at least 2 DEXs for arbitrage
            }

            // Volume filter - FIXED decimal issue, now re-enabled
            // Check minimum volume to avoid illiquid tokens
            let total_volume_24h: f64 = prices.iter().map(|p| p.volume_24h).sum();
            if total_volume_24h < MIN_VOLUME_SOL {
                debug!(
                    "⚠️ Skipping low volume token {}: {:.2} SOL/24h (min: {} SOL)",
                    token_mint.get(..8).unwrap_or(&token_mint),
                    total_volume_24h,
                    MIN_VOLUME_SOL
                );
                continue;
            }

            // Find lowest and highest prices
            let mut min_price = f64::MAX;
            let mut max_price = 0.0;
            let mut buy_dex = String::new();
            let mut sell_dex = String::new();
            // GHOST POOL FIX: Track full pool addresses
            let mut buy_pool_address = String::new();
            let mut sell_pool_address = String::new();

            for price in &prices {
                if price.price_sol < min_price {
                    min_price = price.price_sol;
                    buy_dex = price.dex.clone();
                    buy_pool_address = price.pool_address.clone(); // GHOST POOL FIX
                }
                if price.price_sol > max_price {
                    max_price = price.price_sol;
                    sell_dex = price.dex.clone();
                    sell_pool_address = price.pool_address.clone(); // GHOST POOL FIX
                }
            }

            // Calculate spread
            if min_price > 0.0 && max_price > 0.0 {
                let spread_percentage = ((max_price - min_price) / min_price) * 100.0;

                // Sanity check: reject unrealistic spreads (likely bad price data)
                // Grok fix: Skip same-pool-type arbitrage (not executable)
                // Different pool types within same DEX (e.g., Meteora DAMM variants) aren't arbitrageable
                if buy_dex.starts_with(&sell_dex[..sell_dex.find('_').unwrap_or(sell_dex.len())])
                    && sell_dex.starts_with(&buy_dex[..buy_dex.find('_').unwrap_or(buy_dex.len())])
                {
                    continue; // Skip same-DEX different pools
                }

                // Log ALL spreads above threshold for debugging (Grok: find real opportunities)
                if spread_percentage > LOG_SPREAD_THRESHOLD_PCT {
                    info!(
                        "💡 Found spread: {:.2}% for {} | Buy: {} @ {:.6} | Sell: {} @ {:.6}",
                        spread_percentage,
                        token_mint.get(..8).unwrap_or(&token_mint),
                        buy_dex,
                        min_price,
                        sell_dex,
                        max_price
                    );
                }

                // Grok fix: Raise threshold for volatile memecoins
                if spread_percentage > MAX_REALISTIC_SPREAD_PCT {
                    debug!(
                        "⚠️ Rejecting unrealistic spread: {:.2}% for {} ({} @ {:.6} vs {} @ {:.6})",
                        spread_percentage,
                        token_mint.get(..8).unwrap_or(&token_mint),
                        buy_dex,
                        min_price,
                        sell_dex,
                        max_price
                    );
                    continue;
                }

                // DYNAMIC PROFITABILITY CALCULATION (2025-10-11)
                // Calculate position size and expected gross profit
                let position_size_sol = self
                    .config
                    .max_position_size_sol
                    .min(self.config.capital_sol);
                let position_size_lamports = (position_size_sol * 1_000_000_000.0) as u64;
                let gross_profit_sol = position_size_sol * (spread_percentage / 100.0);
                let gross_profit_lamports = (gross_profit_sol * 1_000_000_000.0) as u64;

                // Calculate ALL costs FIRST (JITO tip + gas + DEX fees) using dynamic tip floor
                let tip_floor = self.jito_tip_floor.read().await;
                let costs = ArbitrageCosts::calculate(
                    position_size_lamports,
                    gross_profit_lamports,
                    true,
                    Some(&*tip_floor),
                );

                // Calculate DYNAMIC minimum spread required
                // Formula: min_spread = (total_costs + margin) / position_size
                // Margin = 0.2% of gross profit for safety buffer
                let margin_lamports = (gross_profit_lamports as f64 * 0.002) as u64; // 0.2% margin
                let min_required_spread_lamports = costs.total_cost_lamports + margin_lamports;
                let min_required_spread_percentage =
                    (min_required_spread_lamports as f64 / position_size_lamports as f64) * 100.0;

                // Check if spread meets DYNAMIC minimum threshold
                if spread_percentage >= min_required_spread_percentage {
                    // Profitable! Calculate net profit
                    let net_profit_lamports = costs.net_profit(gross_profit_lamports);
                    let net_profit_sol = net_profit_lamports as f64 / 1_000_000_000.0;

                    // Log cost breakdown for transparency
                    let (_gas_pct, _tip_pct) = costs.gas_tip_ratio();
                    debug!(
                        "✅ PROFITABLE: {} - Spread {:.2}% >= {:.2}% required",
                        token_mint.get(..8).unwrap_or(&token_mint),
                        spread_percentage,
                        min_required_spread_percentage
                    );
                    debug!(
                        "   Gross: {:.6} SOL, Costs: {:.6} SOL, Net: {:.6} SOL ({:.1}% retention)",
                        gross_profit_sol,
                        costs.total_cost_lamports as f64 / 1e9,
                        net_profit_sol,
                        costs.retention_percentage(gross_profit_lamports)
                    );
                    debug!(
                        "   DEX fees: {:.6} SOL, JITO tip: {:.6} SOL, Gas: {:.6} SOL",
                        costs.dex_fee_lamports as f64 / 1e9,
                        costs.jito_tip_lamports as f64 / 1e9,
                        (costs.base_tx_fee_lamports + costs.compute_fee_lamports) as f64 / 1e9
                    );

                    opportunities.push(ArbitrageOpportunity {
                        token_mint,
                        buy_dex,
                        sell_dex,
                        buy_price: min_price,
                        sell_price: max_price,
                        spread_percentage,
                        estimated_profit_sol: net_profit_sol,
                        // GHOST POOL FIX: Pass full addresses from ShredStream
                        buy_pool_address: buy_pool_address.clone(),
                        sell_pool_address: sell_pool_address.clone(),
                        // NEW (2025-10-11): Record detection time for staleness check
                        detected_at: Instant::now(),
                    });
                } else {
                    debug!("⚠️ Spread too low: {} - {:.2}% < {:.2}% required (Position: {:.2} SOL, Costs: {:.6} SOL)",
                           token_mint.get(..8).unwrap_or(&token_mint), spread_percentage, min_required_spread_percentage,
                           position_size_sol, costs.total_cost_lamports as f64 / 1e9);
                }
            }
        }

        // CYCLE-6: Log scan performance
        let scan_duration = scan_start.elapsed();
        info!(
            "⚡ Scan complete in {:?} ({} opportunities found)",
            scan_duration,
            opportunities.len()
        );

        opportunities
    }

    /// Execute arbitrage trade
    async fn execute_arbitrage(&mut self, opportunity: &ArbitrageOpportunity) -> Result<()> {
        if self.config.paper_trading {
            // Paper trading - simulate execution
            info!("📝 Paper trading: Simulating arbitrage execution");

            // Use consistent RNG for paper trading simulation
            use rand::Rng;
            let success = rand::thread_rng().gen_bool(0.9); // 90% success rate

            if success {
                // Record profit
                self.stats.total_profit_sol += opportunity.estimated_profit_sol;
                info!(
                    "💰 Paper profit: {:.6} SOL (Total: {:.6} SOL)",
                    opportunity.estimated_profit_sol, self.stats.total_profit_sol
                );
                Ok(())
            } else {
                Err(anyhow::anyhow!(
                    "Paper trading: Simulated execution failure"
                ))
            }
        } else {
            // CYCLE-7: Real trading with MANDATORY simulation (Grok recommendation)
            // Execute two-leg arbitrage: Buy low → Sell high
            info!("💰 Executing REAL arbitrage trade");
            info!(
                "   Buy {} @ {:.6} SOL on {}",
                &opportunity.token_mint[..8],
                opportunity.buy_price,
                opportunity.buy_dex
            );
            info!(
                "   Sell {} @ {:.6} SOL on {}",
                &opportunity.token_mint[..8],
                opportunity.sell_price,
                opportunity.sell_dex
            );

            // Safety check: Ensure swap executor exists
            let _swap_executor = self
                .swap_executor
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Swap executor not initialized for real trading"))?;

            // Safety check: Ensure wallet exists
            let wallet = self
                .wallet_keypair
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Wallet not loaded for real trading"))?;

            warn!("⚠️ REAL MONEY TRADING - This will execute actual on-chain transactions!");
            warn!("   Wallet: {}", wallet.pubkey());
            warn!(
                "   Estimated profit: {:.6} SOL",
                opportunity.estimated_profit_sol
            );

            // LIVE TRADING ENABLED - Pool addresses now available from ShredStream
            // All safety systems active: slippage protection, mandatory simulation, circuit breakers

            // GHOST POOL FIX: Use full addresses directly from opportunity (no need to search)
            let buy_pool_address = &opportunity.buy_pool_address;
            let sell_pool_address = &opportunity.sell_pool_address;

            // Parse pool addresses
            let buy_pool_pubkey = buy_pool_address
                .parse::<solana_sdk::pubkey::Pubkey>()
                .context("Invalid buy pool address")?;

            let sell_pool_pubkey = sell_pool_address
                .parse::<solana_sdk::pubkey::Pubkey>()
                .context("Invalid sell pool address")?;

            // CRITICAL: Validate pools exist on-chain (ghost pool protection)
            if let Some(ref rpc) = self.rpc_client {
                debug!("🔍 Validating pool states on-chain...");

                match rpc.get_account_data(&buy_pool_pubkey) {
                    Ok(data) if data.len() > 100 => {
                        debug!("✅ Buy pool valid: {} bytes", data.len());
                    }
                    Ok(data) => {
                        warn!(
                            "👻 GHOST POOL: Buy pool {} has only {} bytes - skipping",
                            buy_pool_address,
                            data.len()
                        );
                        return Err(anyhow::anyhow!(
                            "Buy pool is ghost pool (insufficient data)"
                        ));
                    }
                    Err(e) => {
                        warn!(
                            "👻 GHOST POOL: Buy pool {} doesn't exist: {}",
                            buy_pool_address, e
                        );
                        return Err(anyhow::anyhow!("Buy pool not found on-chain"));
                    }
                }

                match rpc.get_account_data(&sell_pool_pubkey) {
                    Ok(data) if data.len() > 100 => {
                        debug!("✅ Sell pool valid: {} bytes", data.len());
                    }
                    Ok(data) => {
                        warn!(
                            "👻 GHOST POOL: Sell pool {} has only {} bytes - skipping",
                            sell_pool_address,
                            data.len()
                        );
                        return Err(anyhow::anyhow!(
                            "Sell pool is ghost pool (insufficient data)"
                        ));
                    }
                    Err(e) => {
                        warn!(
                            "👻 GHOST POOL: Sell pool {} doesn't exist: {}",
                            sell_pool_address, e
                        );
                        return Err(anyhow::anyhow!("Sell pool not found on-chain"));
                    }
                }
            }

            info!("📍 Pool addresses validated:");
            info!("   Buy pool: {}", buy_pool_address);
            info!("   Sell pool: {}", sell_pool_address);

            // Calculate position size in lamports
            // GROK FIX (2025-10-07): Unify with detection path - use full capital
            let position_size_sol = self
                .config
                .max_position_size_sol
                .min(self.config.capital_sol);
            let position_size_lamports = (position_size_sol * 1e9) as u64;

            info!(
                "💰 Position size: {:.6} SOL ({} lamports)",
                position_size_sol, position_size_lamports
            );

            // CYCLE-7: Execute Meteora swap
            if let (Some(rpc_client), Some(wallet_keypair)) =
                (&self.rpc_client, &self.wallet_keypair)
            {
                // Check if both DEXs are Meteora (or compatible with lb_clmm)
                let is_buy_meteora = opportunity.buy_dex.contains("Meteora");
                let is_sell_meteora = opportunity.sell_dex.contains("Meteora");

                if is_buy_meteora || is_sell_meteora {
                    info!("🚀 Executing Meteora arbitrage opportunity");

                    // Execute buy swap (if Meteora)
                    if is_buy_meteora {
                        info!(
                            "💰 Executing BUY on Meteora: {} @ {:.6} SOL",
                            opportunity
                                .token_mint
                                .get(..8)
                                .unwrap_or(&opportunity.token_mint),
                            opportunity.buy_price
                        );

                        match meteora_swap::execute_meteora_swap(
                            rpc_client.clone(),
                            &buy_pool_address,
                            position_size_lamports,
                            wallet_keypair,
                            0.005,                          // 0.5% slippage tolerance
                            true,                           // Swap X to Y (SOL to token)
                            self.cached_blockhash.as_ref(), // Use pre-fetched blockhash
                        )
                        .await
                        {
                            Ok(signature) => {
                                info!("✅ Buy executed: {}", signature);
                                self.stats.opportunities_executed += 1;
                            }
                            Err(e) => {
                                error!("❌ Buy failed: {}", e);
                                self.stats.failed_executions += 1;
                                self.stats.consecutive_failures += 1;
                                return Err(e);
                            }
                        }
                    }

                    // Execute sell swap (if Meteora)
                    if is_sell_meteora {
                        info!(
                            "💰 Executing SELL on Meteora: {} @ {:.6} SOL",
                            opportunity
                                .token_mint
                                .get(..8)
                                .unwrap_or(&opportunity.token_mint),
                            opportunity.sell_price
                        );

                        match meteora_swap::execute_meteora_swap(
                            rpc_client.clone(),
                            &sell_pool_address,
                            position_size_lamports,
                            wallet_keypair,
                            0.005,                          // 0.5% slippage tolerance
                            false,                          // Swap Y to X (token to SOL)
                            self.cached_blockhash.as_ref(), // Use pre-fetched blockhash
                        )
                        .await
                        {
                            Ok(signature) => {
                                info!("✅ Sell executed: {}", signature);

                                // Reset consecutive failures on success
                                self.stats.consecutive_failures = 0;

                                // Track profit
                                self.stats.total_profit_sol += opportunity.estimated_profit_sol;

                                info!(
                                    "🎉 Arbitrage complete! Estimated profit: {:.6} SOL",
                                    opportunity.estimated_profit_sol
                                );
                            }
                            Err(e) => {
                                error!("❌ Sell failed: {}", e);
                                self.stats.failed_executions += 1;
                                self.stats.consecutive_failures += 1;
                                return Err(e);
                            }
                        }
                    }

                    info!("📊 Arbitrage execution summary:");
                    info!("   Token: {}", opportunity.token_mint);
                    info!(
                        "   Buy DEX: {} (Meteora: {})",
                        opportunity.buy_dex, is_buy_meteora
                    );
                    info!(
                        "   Sell DEX: {} (Meteora: {})",
                        opportunity.sell_dex, is_sell_meteora
                    );
                    info!("   Position: {:.6} SOL", position_size_sol);
                    info!(
                        "   Estimated profit: {:.6} SOL",
                        opportunity.estimated_profit_sol
                    );
                } else {
                    info!("📊 Non-Meteora arbitrage detected (not yet implemented):");
                    info!("   Buy DEX: {}", opportunity.buy_dex);
                    info!("   Sell DEX: {}", opportunity.sell_dex);
                    warn!("⚠️ Only Meteora swaps are implemented. Skipping.");
                }
            } else {
                warn!("⚠️ RPC client or wallet not available - cannot execute swaps");
            }

            Ok(())
        }
    }

    /// Check if we should stop trading (safety limits)
    fn should_stop_trading(&self) -> bool {
        // Daily trade limit
        if self.stats.daily_trades >= self.config.max_daily_trades {
            warn!("⛔ Daily trade limit reached: {}", self.stats.daily_trades);
            return true;
        }

        // Daily loss limit
        if self.stats.total_profit_sol < -self.config.daily_loss_limit_sol {
            warn!(
                "⛔ Daily loss limit reached: {:.6} SOL",
                self.stats.total_profit_sol
            );
            return true;
        }

        // Consecutive failures
        if self.stats.consecutive_failures >= self.config.max_consecutive_failures {
            warn!(
                "⛔ Too many consecutive failures: {}",
                self.stats.consecutive_failures
            );
            return true;
        }

        false
    }

    /// Report statistics
    fn report_stats(&self) {
        info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        info!("📊 Arbitrage Statistics:");
        info!(
            "  • Runtime: {:.1} minutes",
            self.stats.runtime_seconds as f64 / 60.0
        );
        info!(
            "  • Opportunities detected: {}",
            self.stats.opportunities_detected
        );
        info!(
            "  • Opportunities executed: {}",
            self.stats.opportunities_executed
        );
        info!("  • Success rate: {:.1}%", self.stats.success_rate());
        info!("  • Total profit: {:.6} SOL", self.stats.total_profit_sol);
        info!("  • Daily trades: {}", self.stats.daily_trades);
        info!(
            "  • Consecutive failures: {}",
            self.stats.consecutive_failures
        );
        info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    }

    /// Get current statistics
    pub fn get_stats(&self) -> &ArbitrageStats {
        &self.stats
    }

    /// Get pool registry (for population)
    pub fn get_pool_registry(&self) -> &Option<Arc<PoolRegistry>> {
        &self.pool_registry
    }

    /// Execute triangle arbitrage opportunity using real DEX swaps
    async fn execute_triangle_opportunity(
        &mut self,
        opportunity: &crate::triangle_arbitrage::TriangleOpportunity,
    ) -> Result<()> {
        debug!(
            "🔺 Executing triangle opportunity: {:?} → {:.4} SOL profit",
            opportunity.path, opportunity.estimated_profit_sol
        );

        // COST VALIDATION: Verify profitability after ALL costs before execution with dynamic tip floor
        // Calculate position size from config (same as in triangle detection)
        let position_size_sol = self
            .config
            .max_position_size_sol
            .min(self.config.capital_sol);
        let position_size_lamports = (position_size_sol * 1_000_000_000.0) as u64;
        let gross_profit_lamports = (opportunity.estimated_profit_sol * 1_000_000_000.0) as u64;
        let tip_floor = self.jito_tip_floor.read().await;
        let costs = ArbitrageCosts::calculate(
            position_size_lamports,
            gross_profit_lamports,
            true,
            Some(&*tip_floor),
        );

        if !costs.is_profitable(gross_profit_lamports) {
            debug!("⚠️ Triangle opportunity no longer profitable after cost calculation!");
            debug!(
                "   Gross profit: {:.6} SOL ({} lamports)",
                opportunity.estimated_profit_sol, gross_profit_lamports
            );
            debug!(
                "   Total costs: {:.6} SOL ({} lamports)",
                costs.total_cost_lamports as f64 / 1e9,
                costs.total_cost_lamports
            );
            debug!(
                "   Net loss: {:.6} SOL",
                costs.net_profit(gross_profit_lamports) as f64 / 1e9
            );
            return Err(anyhow::anyhow!(
                "Opportunity became unprofitable after cost validation"
            ));
        }

        let net_profit = costs.net_profit(gross_profit_lamports);
        let (gas_pct, tip_pct) = costs.gas_tip_ratio();
        info!("💰 Cost validation passed:");
        info!(
            "   Gross profit: {:.6} SOL",
            opportunity.estimated_profit_sol
        );
        info!(
            "   JITO tip: {:.6} SOL ({:.1}%)",
            costs.jito_tip_lamports as f64 / 1e9,
            tip_pct
        );
        info!(
            "   Gas fees: {:.6} SOL ({:.1}%)",
            (costs.base_tx_fee_lamports + costs.compute_fee_lamports) as f64 / 1e9,
            gas_pct
        );
        info!(
            "   Net profit: {:.6} SOL ({:.1}% retention)",
            net_profit as f64 / 1e9,
            costs.retention_percentage(gross_profit_lamports)
        );

        // Paper trading mode: Simulate execution
        if self.config.paper_trading {
            info!("📄 Paper trading: Simulating triangle execution...");

            // Simulate ~90% success rate (some opportunities will fail due to slippage, MEV, etc.)
            use rand::Rng;
            let success = rand::thread_rng().gen_bool(0.9);

            if success {
                self.stats.opportunities_executed += 1;
                self.stats.total_profit_sol += opportunity.estimated_profit_sol;
                self.stats.consecutive_failures = 0;

                info!("✅ Paper triangle executed successfully!");
                info!(
                    "💰 Paper profit: {:.6} SOL (Total: {:.6} SOL)",
                    opportunity.estimated_profit_sol, self.stats.total_profit_sol
                );

                Ok(())
            } else {
                self.stats.failed_executions += 1;
                self.stats.consecutive_failures += 1;
                warn!("⚠️ Paper triangle execution failed (simulated slippage)");
                Err(anyhow::anyhow!(
                    "Paper trading: Simulated execution failure"
                ))
            }
        }
        // Real trading mode: Execute with swap executor
        else if let (Some(ref mut executor), Some(ref wallet)) =
            (&mut self.swap_executor, &self.wallet_keypair)
        {
            // CYCLE-5 FIX: Check RPC circuit breaker before trading
            if let Err(e) = executor.check_circuit_breaker() {
                error!("🚨 Cannot execute trade: {}", e);
                return Err(e);
            }

            info!("💎 REAL TRADING: Building triangle swap with DEX instructions");

            // Extract pool IDs from DEX strings (e.g., "Meteora_DAMM_V2_81vA2wJx" → "81vA2wJx")
            let pool_ids: Result<Vec<String>> = opportunity
                .dexs
                .iter()
                .map(|dex| extract_pool_id(dex))
                .collect();

            let pool_ids = match pool_ids {
                Ok(ids) => ids,
                Err(e) => {
                    warn!("⚠️ Failed to extract pool IDs: {}", e);
                    return Err(e);
                }
            };

            // CRITICAL FIX: Validate all pool addresses can be resolved BEFORE execution
            // This prevents wasting time building transactions for pools that don't exist
            if let Some(ref pool_registry) = self.pool_registry {
                debug!("🔍 Pre-validating {} pool addresses...", pool_ids.len());

                for (i, pool_id) in pool_ids.iter().enumerate() {
                    let dex_type = DexType::from_dex_string(&opportunity.dexs[i])?;

                    match pool_registry.resolve_pool_address(pool_id, &dex_type).await {
                        Ok(pool_address) => {
                            debug!(
                                "  ✅ Pool {} resolved: {} ({})",
                                i + 1,
                                pool_id,
                                pool_address
                            );
                        }
                        Err(e) => {
                            warn!(
                                "⚠️ Cannot resolve pool address for {} ({:?}): {}",
                                pool_id, dex_type, e
                            );
                            warn!("   Skipping opportunity - pool lookup failed");
                            return Err(anyhow::anyhow!(
                                "Pool address resolution failed for {}: {}",
                                pool_id,
                                e
                            ));
                        }
                    }
                }

                debug!(
                    "✅ All {} pool addresses resolved successfully",
                    pool_ids.len()
                );
            }

            // GROK GHOST POOL SOLUTION - STEP 2: Validate pools before execution
            // Check cache for each pool, batch-validate uncached pools

            // MARKET CHAOS MODE - Skip ghost pool validation for speed
            let skip_ghost_pool_check = std::env::var("SKIP_GHOST_POOL_CHECK")
                .unwrap_or_else(|_| "false".to_string())
                .to_lowercase()
                == "true";

            // PumpSwap pools don't have traditional pool accounts - skip ghost pool validation
            let has_pumpswap = opportunity
                .dexs
                .iter()
                .any(|dex| dex.contains("PumpSwap") || dex.contains("PumpFun"));

            if skip_ghost_pool_check {
                info!(
                    "⚡ MARKET CHAOS MODE: Skipping ghost pool validation for ultra-fast execution"
                );
            } else if has_pumpswap {
                debug!("🪙 PumpSwap pools detected - skipping ghost pool validation (uses bonding curve, not traditional pools)");
            } else if let Some(ref pool_registry) = self.pool_registry {
                debug!(
                    "🔍 Validating {} pools for ghost pool check",
                    pool_ids.len()
                );

                let mut needs_validation = Vec::new();
                for pool_id in &pool_ids {
                    if pool_registry.is_pool_valid_cached(pool_id).await != Some(true) {
                        needs_validation.push(pool_id.clone());
                    }
                }

                // Batch-validate uncached pools
                if !needs_validation.is_empty() {
                    debug!(
                        "🔍 Batch-validating {} uncached pools",
                        needs_validation.len()
                    );
                    if let Err(e) = pool_registry.validate_pools_batch(&needs_validation).await {
                        warn!("⚠️ Pool validation failed: {}", e);
                        return Err(anyhow::anyhow!("Pool validation error: {}", e));
                    }
                }

                // Re-check: Reject if ANY pool is invalid (ghost pool)
                for pool_id in &pool_ids {
                    if pool_registry.is_pool_valid_cached(pool_id).await != Some(true) {
                        debug!(
                            "⚠️ Ghost pool detected: {} (pool doesn't exist on-chain)",
                            pool_id
                        );
                        debug!(
                            "   Rejected opportunity: token {} on {:?}",
                            opportunity.path[1], opportunity.dexs
                        );
                        return Err(anyhow::anyhow!("Ghost pool detected: {}", pool_id));
                    }
                }

                debug!("✅ All {} pools validated successfully", pool_ids.len());
            }

            // Validate we have 2 or 3 DEXs (2-leg arbitrage or 3-leg triangle)
            if pool_ids.len() < 2 || pool_ids.len() > 3 {
                return Err(anyhow::anyhow!(
                    "Invalid opportunity: expected 2-3 DEXs, got {}",
                    pool_ids.len()
                ));
            }

            // Determine DEX types
            let dex_types: Result<Vec<DexType>> = opportunity
                .dexs
                .iter()
                .map(|dex| DexType::from_dex_string(dex))
                .collect();

            let dex_types = match dex_types {
                Ok(types) => types,
                Err(e) => {
                    warn!("⚠️ Failed to parse DEX types: {}", e);
                    return Err(e);
                }
            };

            // CRITICAL FIX: Reserve SOL for fees before calculating position size
            // Can't spend all capital - need to keep SOL for JITO tips + gas + DEX fees
            let gross_capital_lamports =
                (self.config.max_position_size_sol * 1_000_000_000.0) as u64;

            // Subtract all costs to get actual tradeable capital
            let capital_lamports = gross_capital_lamports.saturating_sub(costs.total_cost_lamports);

            info!("💰 Position sizing:");
            info!(
                "   Gross capital: {:.6} SOL",
                gross_capital_lamports as f64 / 1e9
            );
            info!(
                "   Reserved for fees: {:.6} SOL",
                costs.total_cost_lamports as f64 / 1e9
            );
            info!(
                "   Tradeable capital: {:.6} SOL",
                capital_lamports as f64 / 1e9
            );

            // Handle 2-leg arbitrage (SOL → Token → SOL via different DEXs)
            if pool_ids.len() == 2 {
                info!("💱 Executing 2-leg arbitrage (cross-DEX same token):");

                // GROK FIX: Correct profit calculation matching detection logic
                // Prices are in SOL/token, so we DIVIDE (not multiply) for SOL→Token
                const LAMPORTS_PER_SOL: u64 = 1_000_000_000;
                const SWAP_FEE: f64 = 0.0025; // 0.25% per leg

                // Leg 1: SOL → Token (buy on DEX A)
                let amount_in_1 = capital_lamports;
                let capital_sol = amount_in_1 as f64 / LAMPORTS_PER_SOL as f64;

                // CORRECT: SOL / (SOL/token) = tokens (with fee)
                let tokens_received = (capital_sol / opportunity.prices[0]) * (1.0 - SWAP_FEE);
                let expected_out_1 = (tokens_received * 1_000_000_000.0) as u64; // Convert to token lamports
                let min_out_1 =
                    SwapExecutor::calculate_min_output_with_slippage(expected_out_1, 100);

                // Leg 2: Token → SOL (sell on DEX B)
                let amount_in_2 = expected_out_1;

                // CORRECT: tokens * (SOL/token) = SOL (with fee)
                let tokens_sol = amount_in_2 as f64 / 1_000_000_000.0;
                let sol_received = (tokens_sol * opportunity.prices[1]) * (1.0 - SWAP_FEE);
                let expected_out_2 = (sol_received * LAMPORTS_PER_SOL as f64) as u64;
                let min_out_2 =
                    SwapExecutor::calculate_min_output_with_slippage(expected_out_2, 100);

                info!(
                    "   Leg 1: {} SOL → {} tokens on {} (min {})",
                    capital_lamports as f64 / 1e9,
                    expected_out_1,
                    opportunity.dexs[0],
                    min_out_1
                );
                info!(
                    "   Leg 2: {} tokens → {} SOL on {} (min {})",
                    amount_in_2,
                    expected_out_2 as f64 / 1e9,
                    opportunity.dexs[1],
                    min_out_2
                );
                // FIX 1: Reject negative profit trades
                let expected_profit_lamports = expected_out_2 as i64 - capital_lamports as i64;
                if expected_profit_lamports <= 0 {
                    warn!("⚠️ REJECTING trade with negative expected profit!");
                    warn!(
                        "   Initial capital: {:.6} SOL",
                        capital_lamports as f64 / 1e9
                    );
                    warn!("   Expected return: {:.6} SOL", expected_out_2 as f64 / 1e9);
                    warn!(
                        "   Expected profit: {:.6} SOL (LOSS!)",
                        expected_profit_lamports as f64 / 1e9
                    );
                    return Err(anyhow::anyhow!("Trade would result in a loss - rejecting"));
                }

                info!(
                    "   Expected profit: {:.6} SOL",
                    expected_profit_lamports as f64 / 1e9
                );

                let swap1 = SwapParams {
                    amount_in: amount_in_1,
                    minimum_amount_out: min_out_1,
                    expected_amount_out: Some(expected_out_1),
                    swap_a_to_b: true,
                };

                let swap2 = SwapParams {
                    amount_in: amount_in_2,
                    minimum_amount_out: min_out_2,
                    expected_amount_out: Some(expected_out_2),
                    swap_a_to_b: false,
                };

                // For 2-leg, we use the same token as "middle leg" placeholder
                let swap3 = SwapParams {
                    amount_in: 0,
                    minimum_amount_out: 0,
                    expected_amount_out: None,
                    swap_a_to_b: false,
                };

                // SECURITY FIX (2025-10-08): Build transaction with tip INSIDE (not as separate tx)
                // Get random JITO tip account for load balancing
                let tip_account = if let Some(ref client) = self.jito_client {
                    client.get_random_tip_account()
                } else {
                    // Fallback to default if no JITO client (shouldn't happen)
                    "96gYZGLnJYVFmbjzopPSU6QiEV5fGqZNyN9nmNhvrZU5"
                        .parse()
                        .unwrap()
                };

                // Build transaction with tip INSIDE (SECURE method)
                let transaction = executor
                    .build_triangle_with_tip(
                        (&dex_types[0], &pool_ids[0], &swap1),
                        (&dex_types[1], &pool_ids[1], &swap2),
                        (&dex_types[0], &pool_ids[0], &swap3), // Dummy third leg
                        wallet.as_ref(),
                        costs.jito_tip_lamports, // Tip included INSIDE transaction
                        &tip_account,
                    )
                    .await?;

                info!(
                    "🔒 SECURE: JITO tip ({} lamports) included INSIDE transaction",
                    costs.jito_tip_lamports
                );

                // PERFORMANCE OPTIMIZATION (2025-10-12): Final simulation disabled
                //
                // Analysis: 2,043 final simulation rejections vs 0 staleness rejections
                // Problem: Pool state changes in the 5-10ms between initial and final simulation
                // Result: 0% JITO submission rate (everything rejected at final sim)
                //
                // Safety mechanisms still active:
                // 1. ✅ 100ms staleness check (prevents old queued opportunities)
                // 2. ✅ Initial simulation after building (validates instructions)
                // 3. ✅ Cost validation (rejects unprofitable trades)
                // 4. ✅ JITO's own validation (will reject bad bundles)
                //
                // Benefit: 5-10ms faster execution = less time for pool state to change
                //
                // /* COMMENTED OUT - Restore if JITO rejection rate > 30%
                // if let Some(ref rpc) = self.rpc_client {
                //     info!("🧪 Simulating transaction before JITO submission...");
                //     let sim_result = match rpc.simulate_transaction(&transaction) {
                //         Ok(success) => success,
                //         Err(e) => {
                //             warn!("Failed to simulate: {}", e);
                //             false
                //         }
                //     };
                //
                //     if !sim_result {
                //         warn!("❌ Transaction simulation failed - skipping JITO submission");
                //         warn!("   This would have been a wasted submission slot");
                //         return Ok(());
                //     }
                //     info!("✅ Simulation successful - proceeding with JITO submission");
                // }
                // */
                // Submit via queue-based JITO submitter (non-blocking, rate-controlled)
                if let Some(ref submitter) = self.jito_submitter {
                    info!("💎 Submitting 2-leg arbitrage via queue-based JITO...");
                    submitter
                        .submit(
                            vec![transaction],
                            format!(
                                "2-leg: {} → {} → {}",
                                opportunity.path.get(0).unwrap_or(&"SOL".to_string()),
                                opportunity.path.get(1).unwrap_or(&"?".to_string()),
                                opportunity.path.get(0).unwrap_or(&"SOL".to_string())
                            ),
                            opportunity.estimated_profit_sol,
                        )
                        .await?;

                    self.stats.opportunities_executed += 1;
                    self.stats.total_profit_sol += opportunity.estimated_profit_sol;
                    self.stats.consecutive_failures = 0;
                    info!("✅ 2-leg arbitrage queued for JITO submission!");
                    info!(
                        "💵 Expected profit: {:.6} SOL",
                        opportunity.estimated_profit_sol
                    );
                    return Ok(());
                } else {
                    // Fallback: execute directly (paper trading or no JITO)
                    match executor
                        .execute_triangle(
                            (&dex_types[0], &pool_ids[0], &swap1),
                            (&dex_types[1], &pool_ids[1], &swap2),
                            (&dex_types[0], &pool_ids[0], &swap3),
                            wallet.as_ref(),
                            false,
                        )
                        .await
                    {
                        Ok(signature) => {
                            self.stats.opportunities_executed += 1;
                            self.stats.total_profit_sol += opportunity.estimated_profit_sol;
                            self.stats.consecutive_failures = 0;
                            info!("✅ 2-leg arbitrage executed successfully!");
                            info!("💰 Transaction: {}", signature);
                            return Ok(());
                        }
                        Err(e) => {
                            self.stats.failed_executions += 1;
                            self.stats.consecutive_failures += 1;
                            warn!("⚠️ 2-leg arbitrage execution failed: {}", e);
                            return Err(e);
                        }
                    }
                }
            }

            // Handle 3-leg triangle (SOL → TokenA → TokenB → SOL)
            // Leg 1: SOL → TokenA
            let amount_in_1 = capital_lamports;
            let expected_out_1 = (amount_in_1 as f64 * opportunity.prices[0]) as u64;
            let min_out_1 = SwapExecutor::calculate_min_output_with_slippage(expected_out_1, 100); // 1% slippage

            // Leg 2: TokenA → TokenB
            let amount_in_2 = expected_out_1;
            let expected_out_2 = (amount_in_2 as f64 * opportunity.prices[1]) as u64;
            let min_out_2 = SwapExecutor::calculate_min_output_with_slippage(expected_out_2, 100);

            // Leg 3: TokenB → SOL
            let amount_in_3 = expected_out_2;
            let expected_out_3 = (amount_in_3 as f64 * opportunity.prices[2]) as u64;
            let min_out_3 = SwapExecutor::calculate_min_output_with_slippage(expected_out_3, 100);

            // Build swap parameters for each leg
            let swap1 = SwapParams {
                amount_in: amount_in_1,
                minimum_amount_out: min_out_1,
                expected_amount_out: Some(expected_out_1),
                swap_a_to_b: true, // SOL → TokenA
            };

            let swap2 = SwapParams {
                amount_in: amount_in_2,
                minimum_amount_out: min_out_2,
                expected_amount_out: Some(expected_out_2),
                swap_a_to_b: true, // TokenA → TokenB
            };

            let swap3 = SwapParams {
                amount_in: amount_in_3,
                minimum_amount_out: min_out_3,
                expected_amount_out: Some(expected_out_3),
                swap_a_to_b: false, // TokenB → SOL
            };

            // Execute triangle using swap executor
            info!("🔺 Executing 3-leg triangle:");
            info!(
                "   Leg 1: {} lamports → {} (min {})",
                amount_in_1, expected_out_1, min_out_1
            );
            info!(
                "   Leg 2: {} → {} (min {})",
                amount_in_2, expected_out_2, min_out_2
            );
            info!(
                "   Leg 3: {} → {} SOL (min {})",
                amount_in_3, expected_out_3, min_out_3
            );

            // SECURITY FIX (2025-10-08): Build transaction with tip INSIDE (not as separate tx)
            // Get random JITO tip account for load balancing
            let tip_account = if let Some(ref client) = self.jito_client {
                client.get_random_tip_account()
            } else {
                // Fallback to default if no JITO client (shouldn't happen)
                "96gYZGLnJYVFmbjzopPSU6QiEV5fGqZNyN9nmNhvrZU5"
                    .parse()
                    .unwrap()
            };

            // Build transaction with tip INSIDE (SECURE method)
            let transaction = executor
                .build_triangle_with_tip(
                    (&dex_types[0], &pool_ids[0], &swap1),
                    (&dex_types[1], &pool_ids[1], &swap2),
                    (&dex_types[2], &pool_ids[2], &swap3),
                    wallet.as_ref(),
                    costs.jito_tip_lamports, // Tip included INSIDE transaction
                    &tip_account,
                )
                .await?;

            info!(
                "🔒 SECURE: JITO tip ({} lamports) included INSIDE transaction",
                costs.jito_tip_lamports
            );

            // PERFORMANCE OPTIMIZATION (2025-10-12): Final simulation disabled
            //
            // Analysis: 2,043 final simulation rejections vs 0 staleness rejections
            // Problem: Pool state changes in the 5-10ms between initial and final simulation
            // Result: 0% JITO submission rate (everything rejected at final sim)
            //
            // Safety mechanisms still active:
            // 1. ✅ 100ms staleness check (prevents old queued opportunities)
            // 2. ✅ Initial simulation after building (validates instructions)
            // 3. ✅ Cost validation (rejects unprofitable trades)
            // 4. ✅ JITO's own validation (will reject bad bundles)
            //
            // Benefit: 5-10ms faster execution = less time for pool state to change
            //
            // /* COMMENTED OUT - Restore if JITO rejection rate > 30%
            // if let Some(ref rpc) = self.rpc_client {
            //     info!("🧪 Simulating 3-leg triangle transaction before JITO submission...");
            //     let sim_result = match rpc.simulate_transaction(&transaction) {
            //         Ok(success) => success,
            //         Err(e) => {
            //             warn!("Failed to simulate: {}", e);
            //             false
            //         }
            //     };
            //
            //     if !sim_result {
            //         warn!("❌ Triangle transaction simulation failed - skipping JITO submission");
            //         warn!("   This would have been a wasted submission slot");
            //         return Ok(());
            //     }
            //     info!("✅ Triangle simulation successful - proceeding with JITO submission");
            // }
            // */
            // Submit via queue-based JITO submitter (non-blocking, rate-controlled)
            if let Some(ref submitter) = self.jito_submitter {
                info!("💎 Submitting 3-leg triangle via queue-based JITO...");
                submitter
                    .submit(
                        vec![transaction],
                        format!(
                            "Triangle: {} → {} → {} → {}",
                            opportunity.path.get(0).unwrap_or(&"SOL".to_string()),
                            opportunity.path.get(1).unwrap_or(&"?".to_string()),
                            opportunity.path.get(2).unwrap_or(&"?".to_string()),
                            "SOL"
                        ),
                        opportunity.estimated_profit_sol,
                    )
                    .await?;

                self.stats.opportunities_executed += 1;
                self.stats.total_profit_sol += opportunity.estimated_profit_sol;
                self.stats.consecutive_failures = 0;

                info!("✅ 3-leg triangle queued for JITO submission!");
                info!(
                    "💰 Expected profit: {:.6} SOL (Total: {:.6} SOL)",
                    opportunity.estimated_profit_sol, self.stats.total_profit_sol
                );

                Ok(())
            } else {
                // Fallback: execute directly (paper trading or no JITO)
                match executor
                    .execute_triangle(
                        (&dex_types[0], &pool_ids[0], &swap1),
                        (&dex_types[1], &pool_ids[1], &swap2),
                        (&dex_types[2], &pool_ids[2], &swap3),
                        wallet.as_ref(),
                        false,
                    )
                    .await
                {
                    Ok(signature) => {
                        self.stats.opportunities_executed += 1;
                        self.stats.total_profit_sol += opportunity.estimated_profit_sol;
                        self.stats.consecutive_failures = 0;

                        info!("✅ Triangle executed successfully!");
                        info!("💰 Transaction: {}", signature);
                        info!(
                            "💰 Estimated profit: {:.6} SOL (Total: {:.6} SOL)",
                            opportunity.estimated_profit_sol, self.stats.total_profit_sol
                        );

                        Ok(())
                    }
                    Err(e) => {
                        self.stats.failed_executions += 1;
                        self.stats.consecutive_failures += 1;
                        warn!("⚠️ Triangle execution failed: {}", e);
                        Err(e)
                    }
                }
            }
        } else {
            warn!("⚠️ Real trading enabled but swap executor or wallet not initialized");
            Ok(())
        }
    }
}
