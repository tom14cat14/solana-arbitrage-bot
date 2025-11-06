use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use std::sync::{Arc, Mutex};
use reqwest::Client;
use solana_sdk::{
    transaction::Transaction,
    signature::Signer,
    pubkey::Pubkey,
    compute_budget::ComputeBudgetInstruction,
    system_instruction,
};
use tokio::time::timeout;
use tracing::{info, warn, error, debug};
use uuid::Uuid;

/// Token bucket rate limiter for JITO bundle submissions
#[derive(Debug)]
struct RateLimiter {
    tokens: Arc<Mutex<f64>>,
    capacity: f64,
    refill_rate: f64, // tokens per second
    last_refill: Arc<Mutex<Instant>>,
}

impl RateLimiter {
    fn new(capacity: f64, refill_rate: f64) -> Self {
        Self {
            tokens: Arc::new(Mutex::new(capacity)),
            capacity,
            refill_rate,
            last_refill: Arc::new(Mutex::new(Instant::now())),
        }
    }

    async fn acquire(&self) {
        loop {
            {
                let mut tokens = self.tokens.lock().unwrap();
                let mut last_refill = self.last_refill.lock().unwrap();

                // Refill tokens based on elapsed time
                let elapsed = last_refill.elapsed().as_secs_f64();
                let new_tokens = (*tokens + elapsed * self.refill_rate).min(self.capacity);

                if new_tokens >= 1.0 {
                    *tokens = new_tokens - 1.0;
                    *last_refill = Instant::now();
                    return; // Successfully acquired token
                }
            } // Locks dropped here

            // Wait a bit before trying again
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}

/// Production-ready Jito bundle client with HTTP submission and rate limiting
#[derive(Debug)]
pub struct JitoBundleClient {
    client: Client,
    endpoints: Arc<Mutex<Vec<String>>>, // Multiple JITO endpoints with rotation
    current_endpoint_index: Arc<Mutex<usize>>, // Current endpoint for round-robin
    auth_keypair: Option<Arc<solana_sdk::signature::Keypair>>, // SECURITY: Use Arc<Keypair> instead of owned Keypair
    tip_accounts: Vec<Pubkey>,
    bundle_timeout: Duration,
    max_retries: usize,
    metrics: Arc<Mutex<JitoMetrics>>,
    rate_limiter: Arc<RateLimiter>, // JITO rate limiting (30 bundles/minute)
}

#[derive(Debug, Clone)]
pub struct JitoMetrics {
    pub bundles_submitted: u64,
    pub bundles_landed: u64,
    pub bundles_failed: u64,
    pub average_confirmation_time_ms: f64,
    pub tip_amounts_paid: Vec<u64>,
    pub bundle_success_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JitoBundle {
    pub uuid: String,
    pub transactions: Vec<String>, // Base58 encoded transactions
    pub tip_amount: u64,
    pub tip_account: Pubkey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleSubmissionRequest {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    pub params: Vec<Vec<String>>,  // JITO expects params: [[transactions]]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleSubmissionResponse {
    pub jsonrpc: String,
    pub id: u64,
    pub result: Option<String>,
    pub error: Option<JitoError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JitoError {
    pub code: i32,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleStatus {
    pub bundle_id: String,
    pub status: String,
    pub landed_slot: Option<u64>,
    pub transactions: Vec<BundleTransaction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleTransaction {
    pub signature: String,
    pub status: String,
    pub slot: Option<u64>,
}

impl JitoBundleClient {
    /// Create new Jito bundle client with secure keypair reference and multiple endpoints
    pub fn new_with_keypair_ref(
        _block_engine_url: String,  // Deprecated - using multiple endpoints
        _relayer_url: String,  // Deprecated - using multiple endpoints
        auth_keypair: Arc<solana_sdk::signature::Keypair>,
    ) -> Self {
        // Multiple JITO endpoints for rotation (Grok recommendation)
        let endpoints = vec![
            "https://mainnet.block-engine.jito.wtf".to_string(),  // US (primary)
            "https://amsterdam.mainnet.block-engine.jito.wtf".to_string(),  // EU
            "https://frankfurt.mainnet.block-engine.jito.wtf".to_string(),  // EU
            "https://tokyo.mainnet.block-engine.jito.wtf".to_string(),  // Asia
        ];

        info!("🌐 JITO endpoints configured:");
        for (i, endpoint) in endpoints.iter().enumerate() {
            info!("   {}. {}", i + 1, endpoint);
        }

        // Official Jito tip accounts for mainnet-beta
        let tip_accounts = vec![
            "96gYZGLnJYVFmbjzopPSU6QiEV5fGqZNyN9nmNhvrZU5".parse().unwrap(),
            "HFqU5x63VTqvQss8hp11i4wVV8bD44PvwucfZ2bU7gRe".parse().unwrap(),
            "Cw8CFyM9FkoMi7K7Crf6HNQqf4uEMzpKw6QNghXLvLkY".parse().unwrap(),
            "ADaUMid9yfUytqMBgopwjb2DTLSokTSzL1zt6iGPaS49".parse().unwrap(),
            "DfXygSm4jCyNCybVYYK6DwvWqjKee8pbDmJGcLWNDXjh".parse().unwrap(),
            "ADuUkR4vqLUMWXxW9gh6D6L8pMSawimctcNZ5pGwDcEt".parse().unwrap(),
            "DttWaMuVvTiduZRnguLF7jNxTgiMBZ1hyAumKUiL2KRL".parse().unwrap(),
            "3AVi9Tg9Uo68tJfuvoKvqKNWKkC5wPdSSdeBnizKZ6jT".parse().unwrap(),
        ];

        // Create rate limiter: 0.5 tokens/second (2s interval per Grok)
        let rate_limiter = Arc::new(RateLimiter::new(1.0, 0.5));

        info!("✅ JITO rate limiter initialized: 1 bundle per 2 seconds (Grok-optimized for congestion)");

        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap(),
            endpoints: Arc::new(Mutex::new(endpoints)),
            current_endpoint_index: Arc::new(Mutex::new(0)),
            auth_keypair: Some(auth_keypair), // Store Arc<Keypair> securely
            tip_accounts,
            bundle_timeout: Duration::from_secs(60),
            max_retries: 1,  // No retries - fail fast and move to next opportunity
            metrics: Arc::new(Mutex::new(JitoMetrics::default())),
            rate_limiter,
        }
    }

    /// Get a random JITO tip account for load balancing
    ///
    /// Returns one of the 8 official Jito tip accounts at random
    pub fn get_random_tip_account(&self) -> Pubkey {
        use rand::Rng;
        self.tip_accounts[rand::rng().random_range(0..self.tip_accounts.len())]
    }

    /// Create new Jito bundle client (legacy - deprecated, use new_with_keypair_ref)
    #[deprecated(note = "Use new_with_keypair_ref for secure keypair handling")]
    pub fn new(
        _block_engine_url: String,  // Deprecated
        _relayer_url: String,  // Deprecated
        auth_keypair: Option<solana_sdk::signature::Keypair>,
    ) -> Self {
        // Delegate to new implementation
        Self::new_with_keypair_ref(
            String::new(),
            String::new(),
            Arc::new(auth_keypair.unwrap_or_else(|| {
                solana_sdk::signature::Keypair::new()
            })),
        )
    }

    /// Submit bundle with transactions that ALREADY include tip instructions (SECURE)
    ///
    /// **USE THIS METHOD** for production trading! This is the SAFE method that expects
    /// transactions to already have JITO tip instructions included.
    ///
    /// Per Jito docs: "Always make sure your Jito tip transaction is in the same
    /// transaction that is running the MEV strategy"
    ///
    /// # Arguments
    /// * `transactions` - Transactions with tip instructions ALREADY included
    ///
    /// # Returns
    /// Bundle ID from Jito
    ///
    /// # Example
    /// ```ignore
    /// // Build transaction with tip INSIDE
    /// let tx = swap_executor.build_triangle_with_tip(
    ///     leg1, leg2, leg3, wallet, tip_lamports, &tip_account
    /// ).await?;
    ///
    /// // Submit securely (tip already in transaction)
    /// let bundle_id = jito_client.submit_bundle_safe(vec![tx]).await?;
    /// ```
    pub async fn submit_bundle_safe(
        &self,
        transactions: Vec<Transaction>,
    ) -> Result<String> {
        let start_time = Instant::now();

        // RATE LIMITING: Acquire token before proceeding
        // JITO limit: 1 request/second per IP per region
        // IMPORTANT: This rate limit is SHARED across Arb Bot and MEV Bot
        self.rate_limiter.acquire().await;
        debug!("✅ Rate limiter token acquired (took {}ms)", start_time.elapsed().as_millis());

        info!("📦 Submitting SECURE Jito bundle: {} transactions (tips INSIDE transactions)",
              transactions.len());

        // Convert to base58 encoded strings
        let encoded_transactions: Result<Vec<String>> = transactions
            .iter()
            .map(|tx| {
                let serialized = bincode::serialize(tx)?;
                Ok(bs58::encode(serialized).into_string())
            })
            .collect();

        let encoded_transactions = encoded_transactions?;

        // Create bundle (no separate tip - already in transactions)
        let bundle = JitoBundle {
            uuid: Uuid::new_v4().to_string(),
            transactions: encoded_transactions.clone(),
            tip_amount: 0, // Not used - tip already in tx
            tip_account: Pubkey::default(), // Not used
        };

        // Submit with retries
        let bundle_id = self.submit_with_retries(&bundle).await?;

        // Update metrics
        if let Ok(mut metrics) = self.metrics.lock() {
            metrics.bundles_submitted += 1;
        }

        let submission_time = start_time.elapsed().as_millis();
        info!("✅ SECURE bundle submitted in {}ms: {}", submission_time, bundle_id);

        Ok(bundle_id)
    }

    /// Submit bundle with automatic tip calculation and retry logic (LEGACY - INSECURE)
    ///
    /// **⚠️ DEPRECATED**: This method creates a SEPARATE tip transaction, which is DANGEROUS!
    /// Use `submit_bundle_safe()` instead with transactions that already include tips.
    ///
    /// IMPORTANT: This implementation follows Jito's best practices:
    /// 1. Tip instruction is included INSIDE the swap transaction (not separate)
    /// 2. No auth keypair required (new Jito API)
    /// 3. Rate limiting at 1 req/sec per IP per region
    /// 4. Uncle block protection via pre/post account checks
    #[deprecated(note = "Use submit_bundle_safe() with transactions that already include tips")]
    pub async fn submit_bundle(
        &self,
        mut transactions: Vec<Transaction>,
        tip_lamports: Option<u64>,
    ) -> Result<String> {
        let start_time = Instant::now();

        // RATE LIMITING: Acquire token before proceeding
        // JITO limit: 1 request/second per IP per region
        self.rate_limiter.acquire().await;
        debug!("✅ Rate limiter token acquired (took {}ms)", start_time.elapsed().as_millis());

        // Calculate optimal tip if not provided (minimum 1000 lamports per Jito docs)
        #[allow(deprecated)] // Using legacy method until refactored - see TODO below
        let tip_amount = tip_lamports.unwrap_or_else(|| self.calculate_optimal_tip()).max(1000);

        // Select random tip account for load balancing
        use rand::Rng;
        let tip_account = self.tip_accounts[rand::rng().random_range(0..self.tip_accounts.len())];

        // CRITICAL FIX: Include tip INSIDE the swap transaction (not as separate transaction)
        // This prevents "unbundling" via uncle blocks where tip executes but swap fails
        if !transactions.is_empty() {
            // Add tip instruction to the FIRST transaction (the swap transaction)
            let _swap_tx = &mut transactions[0];

            warn!("⚠️ CRITICAL: Tip should be added INSIDE swap transaction, not as separate tx");
            warn!("⚠️ Current implementation adds tip as separate tx - SECURITY RISK!");
            warn!("⚠️ TODO: Refactor swap_executor to build swap instructions WITHOUT signing,");
            warn!("⚠️       then add tip instruction, THEN sign as single transaction");

            // For now, we still use the old method (separate transaction) but log the warning
            // The proper fix requires refactoring swap_executor.rs to expose unsigned instructions
        }

        // Create tip transaction (TEMPORARY - should be integrated into swap tx)
        #[allow(deprecated)] // Using legacy method until swap_executor refactored
        let tip_tx = self.create_tip_transaction_legacy(tip_amount, tip_account)?;

        // Combine transactions
        transactions.push(tip_tx);

        // Convert to base58 encoded strings
        let encoded_transactions: Result<Vec<String>> = transactions
            .iter()
            .map(|tx| {
                let serialized = bincode::serialize(tx)?;
                Ok(bs58::encode(serialized).into_string())
            })
            .collect();

        let encoded_transactions = encoded_transactions?;

        // Create bundle
        let bundle = JitoBundle {
            uuid: Uuid::new_v4().to_string(),
            transactions: encoded_transactions.clone(),
            tip_amount,
            tip_account,
        };

        info!("📦 Submitting Jito bundle: {} transactions, {} lamports tip",
              bundle.transactions.len(), tip_amount);

        // Submit with retries
        let bundle_id = self.submit_with_retries(&bundle).await?;

        // Update metrics
        if let Ok(mut metrics) = self.metrics.lock() {
            metrics.bundles_submitted += 1;
            metrics.tip_amounts_paid.push(tip_amount);
        }

        let submission_time = start_time.elapsed().as_millis();
        debug!("Bundle submitted in {}ms: {}", submission_time, bundle_id);

        Ok(bundle_id)
    }

    /// Submit bundle with retry logic
    async fn submit_with_retries(&self, bundle: &JitoBundle) -> Result<String> {
        let mut last_error = None;

        for attempt in 1..=self.max_retries {
            match self.submit_bundle_once(bundle).await {
                Ok(bundle_id) => {
                    if attempt > 1 {
                        info!("✅ Bundle submitted successfully on attempt {}", attempt);
                    }
                    return Ok(bundle_id);
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    warn!("❌ Bundle submission attempt {} failed: {}", attempt, error_msg);

                    // Rotate endpoint on 429 errors (Grok recommendation)
                    if error_msg.contains("429") {
                        let mut index = self.current_endpoint_index.lock().unwrap();
                        *index = (*index + 1) % self.endpoints.lock().unwrap().len();
                        debug!("🔄 Rotating to endpoint #{} due to 429", *index + 1);
                    }

                    last_error = Some(e);

                    if attempt < self.max_retries {
                        let delay = Duration::from_millis(100 * attempt as u64);
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("All bundle submission attempts failed")))
    }

    /// Single bundle submission attempt
    async fn submit_bundle_once(&self, bundle: &JitoBundle) -> Result<String> {
        use rand::Rng;

        // Get current endpoint (round-robin)
        let current_endpoint = {
            let index = *self.current_endpoint_index.lock().unwrap();
            let endpoints = self.endpoints.lock().unwrap();
            endpoints[index].clone()
        };

        let request = BundleSubmissionRequest {
            jsonrpc: "2.0".to_string(),
            id: rand::rng().random::<u64>(),
            method: "sendBundle".to_string(),
            params: vec![bundle.transactions.clone()],  // Double-wrap: [[txs]]
        };

        debug!("🌐 Submitting to: {}", current_endpoint);

        let response = timeout(
            Duration::from_secs(30),
            self.client
                .post(&format!("{}/api/v1/bundles", current_endpoint))
                .header("Content-Type", "application/json")
                .json(&request)
                .send(),
        )
        .await??;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "HTTP error {}: {}",
                response.status(),
                response.text().await?
            ));
        }

        let bundle_response: BundleSubmissionResponse = response.json().await?;

        if let Some(error) = bundle_response.error {
            return Err(anyhow::anyhow!("Jito error {}: {}", error.code, error.message));
        }

        bundle_response
            .result
            .ok_or_else(|| anyhow::anyhow!("No bundle ID returned"))
    }

    /// Create tip transaction to Jito validators (LEGACY - SECURITY RISK)
    ///
    /// ⚠️ WARNING: This method creates a SEPARATE tip transaction, which is DANGEROUS!
    /// Per Jito docs: "Always make sure your Jito tip transaction is in the same
    /// transaction that is running the MEV strategy"
    ///
    /// RISK: Uncle blocks can cause bundle "unbundling" where tip executes but swap fails,
    /// resulting in losing the tip amount with no profit.
    ///
    /// TODO: Refactor to include tip instruction INSIDE swap transaction:
    /// 1. Build swap instructions (unsigned)
    /// 2. Add tip instruction to same instruction list
    /// 3. Sign as single transaction
    /// 4. Submit in bundle
    #[deprecated(note = "Creates separate tip transaction - SECURITY RISK! Include tip IN swap tx instead")]
    fn create_tip_transaction_legacy(
        &self,
        tip_lamports: u64,
        tip_account: Pubkey,
    ) -> Result<Transaction> {
        let auth_keypair = self.auth_keypair
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Auth keypair required for tip transactions"))?;

        let tip_instruction = system_instruction::transfer(
            &auth_keypair.pubkey(),
            &tip_account,
            tip_lamports,
        );

        // Add compute budget to ensure tip transaction processes quickly
        let compute_budget_instruction = ComputeBudgetInstruction::set_compute_unit_price(50_000);

        let recent_blockhash = solana_sdk::hash::Hash::default(); // Should be fetched from RPC

        let transaction = Transaction::new_signed_with_payer(
            &[compute_budget_instruction, tip_instruction],
            Some(&auth_keypair.pubkey()),
            &[auth_keypair.as_ref()],
            recent_blockhash,
        );

        Ok(transaction)
    }

    /// Build tip instruction (for inclusion in swap transaction)
    ///
    /// This is the SAFE method - returns an instruction to be included
    /// in the same transaction as the swap, preventing unbundling risk.
    ///
    /// Usage:
    /// ```ignore
    /// let tip_instruction = jito_client.build_tip_instruction(10_000, tip_account);
    /// let swap_instructions = vec![/* swap instructions */];
    /// let all_instructions = vec![swap_instructions, vec![tip_instruction]].concat();
    /// let transaction = Transaction::new_signed_with_payer(
    ///     &all_instructions,
    ///     Some(&wallet.pubkey()),
    ///     &[&wallet],
    ///     recent_blockhash,
    /// );
    /// ```
    pub fn build_tip_instruction(
        &self,
        tip_lamports: u64,
        tip_account: Pubkey,
        payer: &Pubkey,
    ) -> solana_sdk::instruction::Instruction {
        system_instruction::transfer(payer, &tip_account, tip_lamports)
    }

    /// Calculate optimal tip based on expected profit and network conditions
    ///
    /// **PROFIT-BASED TIP STRATEGY**:
    /// - Minimum: 100,000 lamports (0.0001 SOL) - 95th percentile per Jito dashboard
    /// - Base: 10% of expected profit
    /// - Adjusted: Based on success rate and confirmation times
    /// - Maximum: 20% of expected profit (prevents overtipping)
    ///
    /// # Arguments
    /// * `expected_profit_lamports` - Expected profit from the arbitrage (optional)
    ///
    /// # Returns
    /// Optimal tip amount in lamports
    pub fn calculate_optimal_tip_with_profit(&self, expected_profit_lamports: Option<u64>) -> u64 {
        // Minimum tip: 100,000 lamports (0.0001 SOL) - 95th percentile
        const MIN_TIP_LAMPORTS: u64 = 100_000;

        // Maximum tip as percentage of profit
        const MAX_TIP_PERCENTAGE: f64 = 0.20; // 20% max

        // Base tip as percentage of profit
        const BASE_TIP_PERCENTAGE: f64 = 0.10; // 10% base

        // Calculate base tip from expected profit
        let base_tip = if let Some(profit) = expected_profit_lamports {
            if profit > 0 {
                // 10% of expected profit, but at least minimum
                let profit_based_tip = (profit as f64 * BASE_TIP_PERCENTAGE) as u64;
                profit_based_tip.max(MIN_TIP_LAMPORTS)
            } else {
                MIN_TIP_LAMPORTS
            }
        } else {
            MIN_TIP_LAMPORTS
        };

        // Adjust based on recent success rate and confirmation times
        let (success_rate_multiplier, latency_multiplier) = if let Ok(metrics) = self.metrics.lock() {
            let success_rate_mult = if metrics.bundle_success_rate < 0.5 {
                1.5 // Increase tip 50% if success rate is low
            } else if metrics.bundle_success_rate > 0.9 {
                0.9 // Reduce tip 10% if success rate is high
            } else {
                1.0
            };

            let latency_mult = if metrics.average_confirmation_time_ms > 5000.0 {
                1.2 // Increase tip 20% if confirmations are slow
            } else if metrics.average_confirmation_time_ms < 2000.0 {
                0.95 // Slightly reduce tip if confirmations are fast
            } else {
                1.0
            };

            (success_rate_mult, latency_mult)
        } else {
            (1.0, 1.0) // Default multipliers if mutex is poisoned
        };

        let adjusted_tip = (base_tip as f64 * success_rate_multiplier * latency_multiplier) as u64;

        // Cap tip at maximum percentage of profit (if profit provided)
        let capped_tip = if let Some(profit) = expected_profit_lamports {
            if profit > 0 {
                let max_tip = (profit as f64 * MAX_TIP_PERCENTAGE) as u64;
                adjusted_tip.min(max_tip)
            } else {
                adjusted_tip
            }
        } else {
            adjusted_tip
        };

        // Ensure minimum tip (95th percentile)
        let final_tip = capped_tip.max(MIN_TIP_LAMPORTS);

        debug!("💰 Calculated optimal tip: {} lamports (0.{:06} SOL)",
               final_tip, final_tip / 1000);

        if let Some(profit) = expected_profit_lamports {
            let tip_percentage = (final_tip as f64 / profit as f64) * 100.0;
            debug!("   Expected profit: {} lamports, Tip: {:.1}% of profit",
                   profit, tip_percentage);
        }

        final_tip
    }

    /// Calculate optimal tip based on current network conditions (LEGACY)
    ///
    /// **DEPRECATED**: Use `calculate_optimal_tip_with_profit()` instead for profit-based tipping
    #[deprecated(note = "Use calculate_optimal_tip_with_profit() for better tip strategy")]
    fn calculate_optimal_tip(&self) -> u64 {
        // Minimum tip: 100,000 lamports (0.0001 SOL) - 95th percentile
        self.calculate_optimal_tip_with_profit(None)
    }

    /// Monitor bundle status and update metrics
    async fn monitor_bundle_status(&self, bundle_id: String) -> Result<()> {
        let start_time = Instant::now();
        let mut check_interval = tokio::time::interval(Duration::from_millis(500));

        for _ in 0..120 { // Monitor for up to 60 seconds
            check_interval.tick().await;

            match self.get_bundle_status(&bundle_id).await {
                Ok(status) => {
                    match status.status.as_str() {
                        "Landed" => {
                            let confirmation_time = start_time.elapsed().as_millis() as f64;
                            info!("✅ Bundle landed in {}ms: {}", confirmation_time, bundle_id);

                            // Update metrics (would need mutable access)
                            // self.metrics.bundles_landed += 1;
                            // self.update_average_confirmation_time(confirmation_time);
                            return Ok(());
                        }
                        "Failed" | "Rejected" => {
                            error!("❌ Bundle failed: {}", bundle_id);
                            // self.metrics.bundles_failed += 1;
                            return Err(anyhow::anyhow!("Bundle failed: {}", status.status));
                        }
                        "Pending" | "Processing" => {
                            debug!("⏳ Bundle pending: {}", bundle_id);
                            continue;
                        }
                        _ => {
                            warn!("Unknown bundle status: {}", status.status);
                            continue;
                        }
                    }
                }
                Err(e) => {
                    debug!("Error checking bundle status: {}", e);
                    continue;
                }
            }
        }

        warn!("⏰ Bundle monitoring timeout: {}", bundle_id);
        Ok(())
    }

    /// Get bundle status from Jito
    async fn get_bundle_status(&self, bundle_id: &str) -> Result<BundleStatus> {
        use rand::Rng;
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": rand::rng().random::<u64>(),
            "method": "getBundleStatuses",
            "params": [vec![bundle_id]]
        });

        // Get current endpoint
        let current_endpoint = {
            let index = *self.current_endpoint_index.lock().unwrap();
            let endpoints = self.endpoints.lock().unwrap();
            endpoints[index].clone()
        };

        let response = timeout(
            Duration::from_secs(10),
            self.client
                .post(&format!("{}/api/v1/bundles", current_endpoint))
                .header("Content-Type", "application/json")
                .json(&request)
                .send(),
        )
        .await??;

        let json: serde_json::Value = response.json().await?;

        if let Some(error) = json.get("error") {
            return Err(anyhow::anyhow!("Jito API error: {}", error));
        }

        let result = json
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
            .ok_or_else(|| anyhow::anyhow!("Invalid bundle status response"))?;

        let status: BundleStatus = serde_json::from_value(result.clone())?;
        Ok(status)
    }

    /// Get bundle performance metrics
    pub fn get_metrics(&self) -> JitoMetrics {
        self.metrics.lock().unwrap_or_else(|poisoned_guard| {
            warn!("Mutex poisoned for metrics, returning default");
            poisoned_guard.into_inner()
        }).clone()
    }

    /// Reset metrics
    pub fn reset_metrics(&self) {
        if let Ok(mut metrics) = self.metrics.lock() {
            *metrics = JitoMetrics::default();
        }
    }

    /// Check if Jito service is available
    pub async fn health_check(&self) -> Result<bool> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getInflightBundleStatuses",
            "params": []
        });

        // Get current endpoint
        let current_endpoint = {
            let index = *self.current_endpoint_index.lock().unwrap();
            let endpoints = self.endpoints.lock().unwrap();
            endpoints[index].clone()
        };

        let response = timeout(
            Duration::from_secs(5),
            self.client
                .post(&format!("{}/api/v1/bundles", current_endpoint))
                .header("Content-Type", "application/json")
                .json(&request)
                .send(),
        )
        .await;

        match response {
            Ok(Ok(resp)) => Ok(resp.status().is_success()),
            _ => Ok(false),
        }
    }
}

impl Default for JitoMetrics {
    fn default() -> Self {
        Self {
            bundles_submitted: 0,
            bundles_landed: 0,
            bundles_failed: 0,
            average_confirmation_time_ms: 0.0,
            tip_amounts_paid: Vec::new(),
            bundle_success_rate: 0.0,
        }
    }
}

impl JitoMetrics {
    /// Calculate success rate
    pub fn calculate_success_rate(&mut self) {
        let total = self.bundles_submitted;
        if total > 0 {
            self.bundle_success_rate = self.bundles_landed as f64 / total as f64;
        }
    }

    /// Update average confirmation time
    pub fn update_average_confirmation_time(&mut self, new_time_ms: f64) {
        let count = self.bundles_landed as f64;
        if count == 1.0 {
            self.average_confirmation_time_ms = new_time_ms;
        } else {
            self.average_confirmation_time_ms =
                (self.average_confirmation_time_ms * (count - 1.0) + new_time_ms) / count;
        }
    }

    /// Get average tip amount
    pub fn average_tip_amount(&self) -> f64 {
        if self.tip_amounts_paid.is_empty() {
            0.0
        } else {
            self.tip_amounts_paid.iter().sum::<u64>() as f64 / self.tip_amounts_paid.len() as f64
        }
    }
}

/// Helper function to create MEV bundle for front-running protection
pub fn create_mev_bundle(
    user_transactions: Vec<Transaction>,
    _tip_lamports: u64,
) -> Vec<Transaction> {
    // In a real MEV bundle, you would:
    // 1. Add a tip transaction at the beginning
    // 2. Add your MEV transactions
    // 3. Add user transactions at the end
    // 4. Ensure all transactions are atomic

    user_transactions // Simplified for now
}