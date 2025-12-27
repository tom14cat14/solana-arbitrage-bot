use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;
use tracing::{debug, warn};

/// Jupiter Price API response
#[derive(Debug, Deserialize)]
pub struct JupiterPriceResponse {
    pub data: HashMap<String, JupiterTokenPrice>,
}

/// Jupiter token price data
#[derive(Debug, Deserialize, Clone)]
pub struct JupiterTokenPrice {
    pub id: String,
    pub price: String, // String because it can be very precise
    #[serde(rename = "mintSymbol")]
    pub mint_symbol: Option<String>,
}

/// Jupiter Price API client
pub struct JupiterPriceClient {
    client: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
}

impl JupiterPriceClient {
    /// Create new Jupiter Price API client with ultra endpoint
    pub fn new(api_key: Option<String>) -> Self {
        // Use ultra endpoint if API key provided, otherwise regular
        let base_url = if api_key.is_some() {
            "https://api.jup.ag/ultra/price/v2".to_string()
        } else {
            "https://api.jup.ag/price/v2".to_string()
        };

        Self {
            client: reqwest::Client::new(),
            base_url,
            api_key,
        }
    }

    /// Fetch prices for multiple token mints
    /// Returns HashMap of mint → price in SOL
    pub async fn fetch_prices(&self, mints: &[String]) -> Result<HashMap<String, f64>> {
        if mints.is_empty() {
            return Ok(HashMap::new());
        }

        // Jupiter API accepts comma-separated mints
        let ids = mints.join(",");
        let url = format!("{}?ids={}", self.base_url, ids);

        debug!("📡 Fetching Jupiter prices for {} tokens", mints.len());

        // Build request with API key if provided
        let mut request = self.client.get(&url);
        if let Some(ref key) = self.api_key {
            request = request.header("X-API-Key", key);
        }

        match request.send().await {
            Ok(response) => {
                let price_response: JupiterPriceResponse = response.json().await?;

                let mut prices = HashMap::new();
                for (mint, price_data) in price_response.data {
                    if let Ok(price) = price_data.price.parse::<f64>() {
                        prices.insert(mint, price);
                    }
                }

                debug!("✅ Fetched {} Jupiter prices", prices.len());
                Ok(prices)
            }
            Err(e) => {
                warn!("❌ Failed to fetch Jupiter prices: {}", e);
                Err(anyhow::anyhow!("Jupiter API error: {}", e))
            }
        }
    }

    /// Fetch price for a single token
    pub async fn fetch_price(&self, mint: &str) -> Result<f64> {
        let prices = self.fetch_prices(&[mint.to_string()]).await?;
        prices
            .get(mint)
            .copied()
            .ok_or_else(|| anyhow::anyhow!("Price not found for mint: {}", mint))
    }

    /// Get best price across multiple DEXs for a token
    /// Jupiter aggregates across all DEXs automatically
    pub async fn get_best_price(&self, mint: &str) -> Result<f64> {
        self.fetch_price(mint).await
    }
}

/// Cross-DEX arbitrage opportunity using Jupiter
#[derive(Debug, Clone)]
pub struct JupiterArbitrageOpportunity {
    pub token_mint: String,
    pub shredstream_dex: String,
    pub shredstream_price: f64,
    pub jupiter_price: f64, // Best aggregated price
    pub spread_percentage: f64,
    pub estimated_profit_sol: f64,
    pub direction: String, // "buy_shredstream_sell_jupiter" or vice versa
}

/// Find arbitrage between ShredStream DEX prices and Jupiter aggregated prices
pub async fn find_jupiter_arbitrage(
    shredstream_prices: &HashMap<String, crate::shredstream_client::TokenPrice>,
    jupiter_client: &JupiterPriceClient,
    min_profit_sol: f64,
    min_spread_percentage: f64,
    capital_sol: f64,
) -> Result<Vec<JupiterArbitrageOpportunity>> {
    let mut opportunities = Vec::new();

    // Get unique token mints from ShredStream
    let mut token_mints: Vec<String> = shredstream_prices
        .values()
        .map(|p| p.token_mint.clone())
        .collect();
    token_mints.sort();
    token_mints.dedup();

    debug!(
        "🔍 Comparing {} tokens: ShredStream vs Jupiter",
        token_mints.len()
    );

    // Rate limiting: Jupiter allows 50 requests per 10 seconds
    // To be safe with other bots: limit to 5 batches per scan (5 requests)
    // This gives us 500 tokens checked per scan cycle
    const BATCH_SIZE: usize = 100;
    const MAX_BATCHES_PER_SCAN: usize = 5; // Only 5 requests per scan (well under 50/10sec)

    let mut batch_count = 0;
    for chunk in token_mints.chunks(BATCH_SIZE) {
        if batch_count >= MAX_BATCHES_PER_SCAN {
            debug!(
                "⏸️ Rate limit: stopping at {} tokens ({} batches)",
                batch_count * BATCH_SIZE,
                batch_count
            );
            break;
        }

        let jupiter_prices = match jupiter_client.fetch_prices(chunk).await {
            Ok(prices) => prices,
            Err(e) => {
                warn!("⚠️ Jupiter batch failed: {}", e);
                continue;
            }
        };

        batch_count += 1;

        // Compare each token's prices
        for mint in chunk {
            let jupiter_price = match jupiter_prices.get(mint) {
                Some(price) => *price,
                None => continue,
            };

            // Find ShredStream prices for this token
            for shredstream_price in shredstream_prices.values() {
                if &shredstream_price.token_mint != mint {
                    continue;
                }

                // Calculate spread
                let spread = (jupiter_price - shredstream_price.price_sol).abs();
                let spread_percentage = (spread / shredstream_price.price_sol) * 100.0;

                if spread_percentage < min_spread_percentage {
                    continue;
                }

                // Determine direction and calculate profit
                let (direction, _buy_price, _sell_price) =
                    if jupiter_price > shredstream_price.price_sol {
                        (
                            "buy_shredstream_sell_jupiter",
                            shredstream_price.price_sol,
                            jupiter_price,
                        )
                    } else {
                        (
                            "buy_jupiter_sell_shredstream",
                            jupiter_price,
                            shredstream_price.price_sol,
                        )
                    };

                // Estimate profit (simplified)
                let gross_profit = capital_sol * (spread_percentage / 100.0);
                let fees = capital_sol * 0.006; // Assume 0.3% per side
                let net_profit = gross_profit - fees;

                if net_profit >= min_profit_sol {
                    opportunities.push(JupiterArbitrageOpportunity {
                        token_mint: mint.clone(),
                        shredstream_dex: shredstream_price.dex.clone(),
                        shredstream_price: shredstream_price.price_sol,
                        jupiter_price,
                        spread_percentage,
                        estimated_profit_sol: net_profit,
                        direction: direction.to_string(),
                    });
                }
            }
        }
    }

    // Sort by profit
    opportunities.sort_by(|a, b| {
        b.estimated_profit_sol
            .partial_cmp(&a.estimated_profit_sol)
            .unwrap()
    });

    if !opportunities.is_empty() {
        debug!(
            "🎯 Found {} Jupiter arbitrage opportunities",
            opportunities.len()
        );
    }

    Ok(opportunities)
}
