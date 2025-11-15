use anyhow::{Context, Result};
use serde::Deserialize;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

/// Jupiter Quote API response
#[derive(Debug, Deserialize)]
pub struct JupiterQuoteResponse {
    #[serde(rename = "inputMint")]
    pub input_mint: String,

    #[serde(rename = "outputMint")]
    pub output_mint: String,

    #[serde(rename = "inAmount")]
    pub in_amount: String,

    #[serde(rename = "outAmount")]
    pub out_amount: String,

    #[serde(rename = "routePlan")]
    pub route_plan: Vec<RoutePlanItem>,
}

#[derive(Debug, Deserialize)]
pub struct RoutePlanItem {
    #[serde(rename = "swapInfo")]
    pub swap_info: SwapInfo,
}

#[derive(Debug, Deserialize)]
pub struct SwapInfo {
    #[serde(rename = "ammKey")]
    pub amm_key: String,

    #[serde(rename = "label")]
    pub label: Option<String>,

    #[serde(rename = "inputMint")]
    pub input_mint: String,

    #[serde(rename = "outputMint")]
    pub output_mint: String,

    #[serde(rename = "inAmount")]
    pub in_amount: String,

    #[serde(rename = "outAmount")]
    pub out_amount: String,
}

/// Triangle arbitrage opportunity found via Jupiter
#[derive(Debug, Clone)]
pub struct JupiterTriangleOpportunity {
    pub input_amount_sol: f64,
    pub output_amount_sol: f64,
    pub profit_sol: f64,
    pub profit_percentage: f64,
    pub route_hops: usize,
    pub route_description: String,
}

/// Rate limiter for Jupiter API
struct RateLimiter {
    requests: Vec<Instant>,
    max_requests: usize,
    window_duration: Duration,
}

impl RateLimiter {
    fn new(max_requests: usize, window_seconds: u64) -> Self {
        Self {
            requests: Vec::new(),
            max_requests,
            window_duration: Duration::from_secs(window_seconds),
        }
    }

    /// Check if we can make a request, and wait if needed
    async fn acquire(&mut self) {
        let now = Instant::now();

        // Remove old requests outside the window
        self.requests
            .retain(|&t| now.duration_since(t) < self.window_duration);

        // If at limit, wait until oldest request expires
        if self.requests.len() >= self.max_requests {
            if let Some(&oldest) = self.requests.first() {
                let wait_time = self
                    .window_duration
                    .checked_sub(now.duration_since(oldest))
                    .unwrap_or(Duration::from_millis(100));

                debug!("⏳ Rate limit reached, waiting {:?}", wait_time);
                tokio::time::sleep(wait_time).await;
            }
        }

        // Add current request
        self.requests.push(Instant::now());
    }
}

/// Jupiter Triangle Arbitrage Detector with rate limiting
pub struct JupiterTriangleDetector {
    client: reqwest::Client,
    api_key: Option<String>,
    rate_limiter: Arc<Mutex<RateLimiter>>,
    sol_mint: String,
}

impl JupiterTriangleDetector {
    pub fn new(api_key: Option<String>) -> Self {
        // Rate limit: 50 requests per 10 seconds (rolling window)
        let rate_limiter = Arc::new(Mutex::new(RateLimiter::new(50, 10)));

        Self {
            client: reqwest::Client::new(),
            api_key,
            rate_limiter,
            sol_mint: "So11111111111111111111111111111111111111112".to_string(),
        }
    }

    /// Find triangle arbitrage opportunities: SOL → ? → ? → SOL
    /// Uses Jupiter's routing to find best multi-hop paths back to SOL
    pub async fn find_triangle_opportunities(
        &self,
        capital_sol: f64,
        config: &crate::config::Config,
    ) -> Result<Vec<JupiterTriangleOpportunity>> {
        let mut opportunities = Vec::new();

        // Convert SOL to lamports for Jupiter API
        let amount_lamports = (capital_sol * 1e9) as u64;

        // Query Jupiter for best SOL → SOL route
        // Jupiter will automatically find multi-hop paths
        if let Some(opportunity) = self.find_sol_to_sol_route(amount_lamports, config).await? {
            opportunities.push(opportunity);
        }

        Ok(opportunities)
    }

    /// Query Jupiter for SOL → SOL route (multi-hop triangle)
    async fn find_sol_to_sol_route(
        &self,
        amount_lamports: u64,
        config: &crate::config::Config,
    ) -> Result<Option<JupiterTriangleOpportunity>> {
        // Acquire rate limit slot
        self.rate_limiter.lock().await.acquire().await;

        // Build Jupiter Ultra API URL
        // Using api.jup.ag/ultra endpoint (requires API key, dynamic rate limits)
        let url = format!(
            "https://api.jup.ag/ultra/v1/order?inputMint={}&outputMint={}&amount={}&taker=11111111111111111111111111111111",
            self.sol_mint,
            self.sol_mint,
            amount_lamports
        );

        debug!("🔍 Querying Jupiter Ultra API for SOL→SOL triangle route");

        // Make request with API key
        let mut request = self.client.get(&url);
        if let Some(ref key) = self.api_key {
            request = request.header("X-API-Key", key);
        }
        let request = request;

        match request.send().await {
            Ok(response) => {
                if !response.status().is_success() {
                    let status = response.status();
                    let text = response.text().await.unwrap_or_default();
                    warn!("❌ Jupiter API error {}: {}", status, text);
                    return Ok(None);
                }

                let quote: JupiterQuoteResponse = response.json().await?;

                // Parse amounts with error context
                let in_amount: u64 = quote.in_amount.parse().context(format!(
                    "Failed to parse Jupiter quote in_amount: {}",
                    quote.in_amount
                ))?;
                let out_amount: u64 = quote.out_amount.parse().context(format!(
                    "Failed to parse Jupiter quote out_amount: {}",
                    quote.out_amount
                ))?;

                let input_sol = in_amount as f64 / 1e9;
                let output_sol = out_amount as f64 / 1e9;
                let gross_profit = output_sol - input_sol;
                let profit_pct = (gross_profit / input_sol) * 100.0;

                // Build route description
                let route_desc = self.build_route_description(&quote.route_plan);

                // Calculate total fees (JITO tip + gas + compute)
                let total_fees = config.calculate_total_fees(gross_profit);
                let net_profit = gross_profit - total_fees;

                // Calculate required margin (UPDATED 2025-10-11)
                // NEW: fees + 0.5% of gross profit (user requirement)
                let required_margin = 0.005 * gross_profit; // 0.5% of gross as safety margin
                let min_acceptable = total_fees + required_margin;

                debug!(
                    "🔺 Jupiter route: {} → Gross={:.6} SOL, Fees={:.6} SOL, Net={:.6} SOL ({:.2}%)",
                    route_desc, gross_profit, total_fees, net_profit, profit_pct
                );

                // Check if profitable with required margin
                if net_profit >= min_acceptable {
                    info!(
                        "🎯 Found Jupiter triangle: {} - Net profit {:.6} SOL after fees ({:.2}%)",
                        route_desc, net_profit, profit_pct
                    );

                    return Ok(Some(JupiterTriangleOpportunity {
                        input_amount_sol: input_sol,
                        output_amount_sol: output_sol,
                        profit_sol: net_profit, // Store NET profit (after all fees)
                        profit_percentage: profit_pct,
                        route_hops: quote.route_plan.len(),
                        route_description: route_desc,
                    }));
                }

                Ok(None)
            }
            Err(e) => {
                warn!("❌ Jupiter API request failed: {}", e);
                Ok(None)
            }
        }
    }

    /// Build human-readable route description
    fn build_route_description(&self, route_plan: &[RoutePlanItem]) -> String {
        let mut parts = Vec::new();

        for item in route_plan {
            if let Some(ref label) = item.swap_info.label {
                parts.push(label.clone());
            } else {
                parts.push("Unknown DEX".to_string());
            }
        }

        if parts.is_empty() {
            "SOL→SOL".to_string()
        } else {
            format!("SOL→{}→SOL ({} hops)", parts.join("→"), parts.len())
        }
    }
}
