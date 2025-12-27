use anyhow::Result;
use std::collections::HashMap;
use std::time::Instant;
use tokio::sync::mpsc;
use tracing::{info, warn, debug};
use chrono::{DateTime, Utc};
use futures::{SinkExt, StreamExt};

use yellowstone_grpc_client::GeyserGrpcClient;
use yellowstone_grpc_proto::prelude::*;
use yellowstone_grpc_proto::prelude::subscribe_update::UpdateOneof;

use crate::dex_registry::DexRegistry;
use crate::dex_transaction_parser::DexTransactionParser;

/// Real-time price update from ERPC gRPC stream
#[derive(Debug, Clone)]
pub struct RealPriceUpdate {
    pub token_mint: String,
    pub dex_program_id: String,
    pub price_sol: f64,
    pub liquidity: u64,
    pub volume_24h: f64,
    pub timestamp: DateTime<Utc>,
    pub confidence: f64,
    pub slot: u64,
}

/// Real-time price feed using ERPC gRPC streaming
pub struct GrpcPriceFeed {
    grpc_endpoint: String,
    x_token: String,
    dex_registry: DexRegistry,
    dex_parser: DexTransactionParser,
    price_cache: HashMap<String, RealPriceUpdate>,
    stats: PriceFeedStats,
}

#[derive(Debug, Clone, Default)]
struct PriceFeedStats {
    connections_established: u64,
    messages_received: u64,
    prices_extracted: u64,
    transactions_processed: u64,
}

impl GrpcPriceFeed {
    pub fn new(grpc_endpoint: String, x_token: String) -> Self {
        info!("🚀 Initializing ERPC gRPC Price Feed");
        info!("  • Endpoint: {}", grpc_endpoint);

        Self {
            grpc_endpoint,
            x_token,
            dex_registry: DexRegistry::new(),
            dex_parser: DexTransactionParser::new(),
            price_cache: HashMap::new(),
            stats: PriceFeedStats::default(),
        }
    }

    /// Start real-time price monitoring with ERPC gRPC streaming
    pub async fn start_real_price_monitoring(&mut self) -> Result<()> {
        info!("🔌 Connecting to ERPC gRPC endpoint: {}", self.grpc_endpoint);

        // Build gRPC client with proper TLS configuration
        let endpoint = if self.grpc_endpoint.starts_with("http") {
            self.grpc_endpoint.clone()
        } else {
            format!("https://{}", self.grpc_endpoint)
        };

        info!("  • Full endpoint URL: {}", endpoint);
        info!("  • Using X-Token authentication");

        let mut client = GeyserGrpcClient::build_from_shared(endpoint.clone())?
            .x_token(Some(self.x_token.clone()))?
            .connect()
            .await?;

        self.stats.connections_established += 1;
        info!("✅ ERPC gRPC connection established successfully");

        // Subscribe to all DEX program accounts for real-time swap detection
        let mut accounts_filter = HashMap::new();

        for (dex_name, dex_info) in &self.dex_registry.dexs {
            debug!("  • Subscribing to {} ({})", dex_name, dex_info.program_id);

            accounts_filter.insert(
                dex_name.clone(),
                SubscribeRequestFilterAccounts {
                    account: vec![],  // Empty = all accounts
                    owner: vec![dex_info.program_id.to_string()],  // Filter by DEX program owner
                    filters: vec![],
                },
            );
        }

        let mut subscribe_request = HashMap::new();
        subscribe_request.insert("client".to_string(), SubscribeRequestFilterAccounts {
            account: vec![],
            owner: vec![],
            filters: vec![],
        });

        info!("📡 Subscribing to {} DEX programs for real-time swaps", accounts_filter.len());

        let (mut subscribe_tx, mut stream) = client.subscribe().await?;

        // Send subscription request
        subscribe_tx.send(SubscribeRequest {
            slots: Default::default(),
            accounts: accounts_filter,
            transactions: Default::default(),
            transactions_status: Default::default(),
            blocks: Default::default(),
            blocks_meta: Default::default(),
            entry: Default::default(),
            commitment: Some(CommitmentLevel::Processed as i32),
            accounts_data_slice: vec![],
            ping: None,
        }).await?;

        info!("🚀 Starting to receive real-time gRPC messages from ERPC...");

        let mut update_count = 0u64;

        // Process gRPC stream
        while let Some(message) = stream.next().await {
            let message = message?;  // Handle the Result
            self.stats.messages_received += 1;

            match message.update_oneof {
                Some(update_oneof) => {
                    match update_oneof {
                        UpdateOneof::Account(account_update) => {
                            // Process account update to extract swap data
                            if let Some(price_update) = self.process_account_update(account_update).await? {
                                self.process_price_update(price_update).await;
                                update_count += 1;

                                if update_count % 100 == 0 {
                                    self.log_performance_stats();
                                }
                            }
                        }
                        UpdateOneof::Transaction(tx_update) => {
                            // Process transaction for swap instructions
                            let price_updates = self.process_transaction_update(tx_update).await?;
                            for price_update in price_updates {
                                self.process_price_update(price_update).await;
                                update_count += 1;
                            }
                        }
                        UpdateOneof::Slot(slot_update) => {
                            debug!("📊 Slot update: {}", slot_update.slot);
                        }
                        _ => {
                            debug!("Other update type received");
                        }
                    }
                }
                None => {
                    debug!("Empty update received");
                }
            }
        }

        warn!("⚠️ gRPC stream ended");
        Ok(())
    }

    /// Process account update from gRPC stream
    async fn process_account_update(&mut self, account: SubscribeUpdateAccount) -> Result<Option<RealPriceUpdate>> {
        self.stats.transactions_processed += 1;

        // Extract account data and parse for DEX-specific information
        // This is where we'd parse pool state changes, liquidity updates, etc.

        debug!("Account update: pubkey={}, slot={}, lamports={}",
               bs58::encode(&account.account.as_ref().unwrap().pubkey).into_string(),
               account.slot,
               account.account.as_ref().unwrap().lamports);

        // For now, return None - full account parsing would go here
        Ok(None)
    }

    /// Process transaction update from gRPC stream
    async fn process_transaction_update(&mut self, tx: SubscribeUpdateTransaction) -> Result<Vec<RealPriceUpdate>> {
        let mut price_updates = Vec::new();
        self.stats.transactions_processed += 1;

        // Extract transaction and parse swap instructions
        if let Some(transaction) = tx.transaction {
            if let Some(tx_data) = transaction.transaction {
                // Parse transaction to extract swap instructions
                // This would use our existing DEX parser

                debug!("Transaction update: slot={}, signature={:?}",
                       tx.slot,
                       transaction.signature);

                // TODO: Parse transaction instructions using dex_parser
                // For now, return empty vec
            }
        }

        Ok(price_updates)
    }

    /// Process and cache price update
    async fn process_price_update(&mut self, price_update: RealPriceUpdate) {
        let cache_key = format!("{}_{}", price_update.token_mint, price_update.dex_program_id);
        self.price_cache.insert(cache_key.clone(), price_update.clone());
        self.stats.prices_extracted += 1;

        info!("💰 Price update: {} on {} = {:.8} SOL (slot: {})",
              &price_update.token_mint[..8],
              &price_update.dex_program_id[..8],
              price_update.price_sol,
              price_update.slot);
    }

    /// Get cached price for a token on a specific DEX
    pub fn get_cached_price(&self, token_mint: &str, dex_program_id: &str) -> Option<RealPriceUpdate> {
        let cache_key = format!("{}_{}", token_mint, dex_program_id);
        self.price_cache.get(&cache_key).cloned()
    }

    /// Get all cached prices
    pub fn get_all_prices(&self) -> Vec<RealPriceUpdate> {
        self.price_cache.values().cloned().collect()
    }

    /// Log performance statistics
    fn log_performance_stats(&self) {
        info!("📊 gRPC Price Feed Stats:");
        info!("  • Connections: {}", self.stats.connections_established);
        info!("  • Messages received: {}", self.stats.messages_received);
        info!("  • Transactions processed: {}", self.stats.transactions_processed);
        info!("  • Prices extracted: {}", self.stats.prices_extracted);
        info!("  • Cached prices: {}", self.price_cache.len());
    }

    /// Get statistics
    pub fn get_stats(&self) -> HashMap<String, u64> {
        let mut stats = HashMap::new();
        stats.insert("connections".to_string(), self.stats.connections_established);
        stats.insert("messages".to_string(), self.stats.messages_received);
        stats.insert("transactions".to_string(), self.stats.transactions_processed);
        stats.insert("prices".to_string(), self.stats.prices_extracted);
        stats.insert("cached".to_string(), self.price_cache.len() as u64);
        stats
    }
}
