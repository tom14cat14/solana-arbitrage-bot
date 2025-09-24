use anyhow::{Result, anyhow};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info, warn};

use crate::config::Config;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JupiterQuoteRequest {
    #[serde(rename = "inputMint")]
    pub input_mint: String,
    #[serde(rename = "outputMint")]
    pub output_mint: String,
    pub amount: String,
    #[serde(rename = "slippageBps")]
    pub slippage_bps: u16,
    #[serde(rename = "onlyDirectRoutes")]
    pub only_direct_routes: Option<bool>,
    #[serde(rename = "asLegacyTransaction")]
    pub as_legacy_transaction: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JupiterQuoteResponse {
    #[serde(rename = "inputMint")]
    pub input_mint: String,
    #[serde(rename = "inAmount")]
    pub in_amount: String,
    #[serde(rename = "outputMint")]
    pub output_mint: String,
    #[serde(rename = "outAmount")]
    pub out_amount: String,
    #[serde(rename = "otherAmountThreshold")]
    pub other_amount_threshold: String,
    #[serde(rename = "swapMode")]
    pub swap_mode: String,
    #[serde(rename = "slippageBps")]
    pub slippage_bps: u16,
    #[serde(rename = "platformFee")]
    pub platform_fee: Option<PlatformFee>,
    #[serde(rename = "priceImpactPct")]
    pub price_impact_pct: String,
    #[serde(rename = "routePlan")]
    pub route_plan: Vec<RoutePlanStep>,
    #[serde(rename = "contextSlot")]
    pub context_slot: Option<u64>,
    #[serde(rename = "timeTaken")]
    pub time_taken: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformFee {
    pub amount: String,
    #[serde(rename = "feeBps")]
    pub fee_bps: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutePlanStep {
    #[serde(rename = "swapInfo")]
    pub swap_info: SwapInfo,
    pub percent: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapInfo {
    #[serde(rename = "ammKey")]
    pub amm_key: String,
    pub label: String,
    #[serde(rename = "inputMint")]
    pub input_mint: String,
    #[serde(rename = "outputMint")]
    pub output_mint: String,
    #[serde(rename = "inAmount")]
    pub in_amount: String,
    #[serde(rename = "outAmount")]
    pub out_amount: String,
    #[serde(rename = "feeAmount")]
    pub fee_amount: String,
    #[serde(rename = "feeMint")]
    pub fee_mint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JupiterSwapRequest {
    #[serde(rename = "quoteResponse")]
    pub quote_response: JupiterQuoteResponse,
    #[serde(rename = "userPublicKey")]
    pub user_public_key: String,
    #[serde(rename = "wrapAndUnwrapSol")]
    pub wrap_and_unwrap_sol: Option<bool>,
    #[serde(rename = "useSharedAccounts")]
    pub use_shared_accounts: Option<bool>,
    #[serde(rename = "feeAccount")]
    pub fee_account: Option<String>,
    #[serde(rename = "trackingAccount")]
    pub tracking_account: Option<String>,
    #[serde(rename = "computeUnitPriceMicroLamports")]
    pub compute_unit_price_micro_lamports: Option<u64>,
    #[serde(rename = "prioritizationFeeLamports")]
    pub prioritization_fee_lamports: Option<u64>,
    #[serde(rename = "asLegacyTransaction")]
    pub as_legacy_transaction: Option<bool>,
    #[serde(rename = "useTokenLedger")]
    pub use_token_ledger: Option<bool>,
    #[serde(rename = "destinationTokenAccount")]
    pub destination_token_account: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JupiterSwapResponse {
    #[serde(rename = "swapTransaction")]
    pub swap_transaction: String,
    #[serde(rename = "lastValidBlockHeight")]
    pub last_valid_block_height: Option<u64>,
    #[serde(rename = "prioritizationFeeLamports")]
    pub prioritization_fee_lamports: Option<u64>,
    #[serde(rename = "computeUnitLimit")]
    pub compute_unit_limit: Option<u64>,
    #[serde(rename = "setupInstructions")]
    pub setup_instructions: Option<Vec<String>>,
    #[serde(rename = "cleanupInstruction")]
    pub cleanup_instruction: Option<String>,
    #[serde(rename = "addressLookupTableAddresses")]
    pub address_lookup_table_addresses: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenInfo {
    pub address: String,
    #[serde(rename = "chainId")]
    pub chain_id: u32,
    pub decimals: u8,
    pub name: String,
    pub symbol: String,
    #[serde(rename = "logoURI")]
    pub logo_uri: Option<String>,
    pub tags: Vec<String>,
    #[serde(rename = "daily_volume")]
    pub daily_volume: Option<f64>,
}

pub struct JupiterClient {
    client: Client,
    config: Config,
    base_url: String,
    tokens: HashMap<String, TokenInfo>,
}

impl JupiterClient {
    pub async fn new(config: Config) -> Result<Self> {
        let client = Client::new();
        let base_url = config.jupiter_endpoint.clone();

        let mut jupiter_client = Self {
            client,
            config,
            base_url,
            tokens: HashMap::new(),
        };

        // Load token list on initialization
        jupiter_client.load_token_list().await?;

        info!("Jupiter client initialized with {} tokens", jupiter_client.tokens.len());

        Ok(jupiter_client)
    }

    async fn load_token_list(&mut self) -> Result<()> {
        let url = format!("{}/tokens", self.base_url);

        let response = self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.config.jupiter_api_key))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Failed to load token list: {}", response.status()));
        }

        let tokens: Vec<TokenInfo> = response.json().await?;

        for token in tokens {
            self.tokens.insert(token.address.clone(), token);
        }

        Ok(())
    }

    pub async fn get_quote(
        &self,
        input_mint: &str,
        output_mint: &str,
        amount: u64,
        slippage_bps: u16,
    ) -> Result<JupiterQuoteResponse> {
        let url = format!("{}/quote", self.base_url);

        let request = JupiterQuoteRequest {
            input_mint: input_mint.to_string(),
            output_mint: output_mint.to_string(),
            amount: amount.to_string(),
            slippage_bps,
            only_direct_routes: Some(false),
            as_legacy_transaction: Some(false),
        };

        debug!("Getting quote: {} {} -> {}", amount, input_mint, output_mint);

        let response = self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.config.jupiter_api_key))
            .query(&[
                ("inputMint", &request.input_mint),
                ("outputMint", &request.output_mint),
                ("amount", &request.amount),
                ("slippageBps", &request.slippage_bps.to_string()),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Quote request failed: {}", error_text));
        }

        let quote: JupiterQuoteResponse = response.json().await?;

        debug!(
            "Quote received: {} -> {} (price impact: {}%)",
            quote.in_amount,
            quote.out_amount,
            quote.price_impact_pct
        );

        Ok(quote)
    }

    pub async fn get_swap_transaction(
        &self,
        quote: JupiterQuoteResponse,
        user_public_key: &str,
        priority_fee_lamports: Option<u64>,
    ) -> Result<JupiterSwapResponse> {
        let url = format!("{}/swap", self.base_url);

        let request = JupiterSwapRequest {
            quote_response: quote,
            user_public_key: user_public_key.to_string(),
            wrap_and_unwrap_sol: Some(true),
            use_shared_accounts: Some(true),
            fee_account: None,
            tracking_account: None,
            compute_unit_price_micro_lamports: None,
            prioritization_fee_lamports: priority_fee_lamports,
            as_legacy_transaction: Some(false),
            use_token_ledger: Some(false),
            destination_token_account: None,
        };

        debug!("Getting swap transaction for user: {}", user_public_key);

        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.jupiter_api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Swap transaction request failed: {}", error_text));
        }

        let swap_response: JupiterSwapResponse = response.json().await?;

        debug!("Swap transaction received");

        Ok(swap_response)
    }

    pub async fn get_best_route_for_amount(
        &self,
        input_mint: &str,
        output_mint: &str,
        amount: u64,
    ) -> Result<Option<JupiterQuoteResponse>> {
        // Try different slippage tolerances to find the best route
        let slippage_options = vec![50, 100, 200, 500]; // 0.5%, 1%, 2%, 5%

        for slippage in slippage_options {
            match self.get_quote(input_mint, output_mint, amount, slippage).await {
                Ok(quote) => {
                    // Check if the price impact is acceptable
                    let price_impact: f64 = quote.price_impact_pct.parse().unwrap_or(100.0);
                    if price_impact < 5.0 { // Less than 5% price impact
                        return Ok(Some(quote));
                    }
                }
                Err(e) => {
                    debug!("Quote failed with slippage {}: {}", slippage, e);
                    continue;
                }
            }
        }

        Ok(None)
    }

    pub async fn health_check(&self) -> Result<()> {
        let url = format!("{}/tokens", self.base_url);

        let response = self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.config.jupiter_api_key))
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Jupiter API health check failed: {}", response.status()));
        }

        Ok(())
    }

    pub fn get_token_info(&self, mint: &str) -> Option<&TokenInfo> {
        self.tokens.get(mint)
    }

    pub fn get_popular_tokens(&self) -> Vec<&TokenInfo> {
        let mut tokens: Vec<&TokenInfo> = self.tokens.values()
            .filter(|token| token.daily_volume.unwrap_or(0.0) > 100000.0) // $100k+ daily volume
            .collect();

        tokens.sort_by(|a, b| {
            b.daily_volume.unwrap_or(0.0)
                .partial_cmp(&a.daily_volume.unwrap_or(0.0))
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        tokens.into_iter().take(50).collect() // Top 50 tokens
    }

    pub async fn calculate_route_efficiency(&self, quote: &JupiterQuoteResponse) -> f64 {
        // Calculate efficiency score based on:
        // - Number of hops (fewer is better)
        // - Price impact (lower is better)
        // - Route complexity

        let num_hops = quote.route_plan.len() as f64;
        let price_impact: f64 = quote.price_impact_pct.parse().unwrap_or(100.0);

        // Efficiency score (0-1, higher is better)
        let hop_penalty = (num_hops - 1.0) * 0.1; // Penalty for each additional hop
        let impact_penalty = price_impact / 100.0; // Convert percentage to decimal

        (1.0 - hop_penalty - impact_penalty).max(0.0).min(1.0)
    }
}