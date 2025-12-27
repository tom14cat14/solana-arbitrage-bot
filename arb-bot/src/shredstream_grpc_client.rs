use anyhow::Result;
use futures::StreamExt;
use solana_stream_sdk::{CommitmentLevel, ShredstreamClient};
use tracing::{info, warn, debug};
use chrono::Utc;
use std::collections::HashMap;

use crate::dex_transaction_parser::DexTransactionParser;

/// Real ShredStream gRPC-over-HTTPS client (WORKING approach from SLV template)
/// Connects to https://shreds-ny6-1.erpc.global
pub struct ShredStreamGrpcClient {
    endpoint: String,
    dex_parser: DexTransactionParser,
    price_cache: HashMap<String, PriceUpdate>,
}

#[derive(Debug, Clone)]
pub struct PriceUpdate {
    pub token_mint: String,
    pub dex_program_id: String,
    pub price_sol: f64,
    pub liquidity: u64,
    pub volume_24h: f64,
    pub timestamp: chrono::DateTime<Utc>,
}

impl ShredStreamGrpcClient {
    pub fn new(endpoint: String) -> Self {
        info!("🌊 Initializing ShredStream gRPC Client (SLV approach)");
        info!("  • Endpoint: {}", endpoint);
        info!("  • Protocol: gRPC-over-HTTPS");
        info!("  • Method: ShredstreamClient::connect()");

        Self {
            endpoint,
            dex_parser: DexTransactionParser::new(),
            price_cache: HashMap::new(),
        }
    }

    /// Start monitoring ShredStream using gRPC-over-HTTPS (WORKING method)
    pub async fn start_monitoring(&mut self) -> Result<()> {
        info!("🔌 Connecting to ShredStream via gRPC-over-HTTPS...");
        info!("   Endpoint: {}", self.endpoint);

        // Connect using solana-stream-sdk (same as SLV template)
        let mut client = ShredstreamClient::connect(&self.endpoint).await?;
        info!("✅ ShredStream gRPC connection established");

        // Subscribe to entries (no filter = all entries)
        let request = ShredstreamClient::create_entries_request_for_accounts(
            vec![],  // No account filter
            vec![],  // No program filter
            vec![],  // No owner filter
            Some(CommitmentLevel::Processed),  // Processed commitment
        );

        info!("📡 Subscribing to ShredStream entries...");
        let mut stream = client.subscribe_entries(request).await?;
        info!("✅ ShredStream subscription active");

        let mut entry_count = 0u64;
        let mut tx_count = 0u64;

        // Process stream (same pattern as SLV template)
        while let Some(slot_entry) = stream.next().await {
            match slot_entry {
                Ok(data) => {
                    entry_count += 1;
                    let slot = data.slot;

                    // Deserialize entries (copied from SLV template comments)
                    match bincode::deserialize::<Vec<solana_entry::entry::Entry>>(&data.entries) {
                        Ok(entries) => {
                            let transactions: Vec<_> = entries
                                .iter()
                                .flat_map(|e| e.transactions.iter())
                                .collect();

                            tx_count += transactions.len() as u64;

                            if entry_count % 100 == 0 {
                                info!(
                                    "📊 ShredStream: {} entries, {} transactions (slot {})",
                                    entry_count, tx_count, slot
                                );
                            }

                            // TODO: Parse transactions for DEX swaps and extract prices
                            // This is where we'd call dex_parser.parse_dex_transactions()
                        }
                        Err(e) => {
                            warn!("Failed to deserialize entries: {}", e);
                        }
                    }
                }
                Err(e) => {
                    warn!("ShredStream error: {:?}", e);
                    break;
                }
            }
        }

        warn!("ShredStream connection closed");
        Ok(())
    }

    /// Process a single cycle (for integration with arbitrage engine)
    pub async fn process_single_cycle(&mut self) -> Result<Vec<PriceUpdate>> {
        // This will be called from the main loop
        // For now, return empty - full implementation needs stream handling
        Ok(Vec::new())
    }

    pub fn get_prices(&self) -> &HashMap<String, PriceUpdate> {
        &self.price_cache
    }
}
