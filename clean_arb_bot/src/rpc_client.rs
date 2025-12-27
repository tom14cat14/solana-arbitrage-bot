// Solana RPC client wrapper for DEX swap operations
//
// Provides a clean interface for:
// - Fetching recent blockhash
// - Simulating transactions
// - Fetching account data
// - Getting pool state information

use anyhow::{Context, Result};
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_config::RpcSimulateTransactionConfig;
use solana_sdk::{
    commitment_config::CommitmentConfig, hash::Hash, pubkey::Pubkey, signature::Signature,
    transaction::Transaction,
};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use tracing::{debug, error, info, trace, warn};

/// CYCLE-5 FIX: RPC circuit breaker threshold
/// Halts trading after this many consecutive RPC failures to prevent losses during network issues
const CIRCUIT_BREAKER_THRESHOLD: u32 = 5;

/// Wrapper around Solana RPC client with convenience methods for DEX operations
/// CYCLE-5 FIX: Added circuit breaker to halt trading during sustained RPC failures
pub struct SolanaRpcClient {
    client: RpcClient,
    commitment: CommitmentConfig,
    consecutive_failures: AtomicU32, // CYCLE-5: Track consecutive RPC failures
}

impl SolanaRpcClient {
    /// Create new RPC client with endpoint URL
    pub fn new(rpc_url: String) -> Self {
        let commitment = CommitmentConfig::confirmed();
        let client = RpcClient::new_with_commitment(rpc_url.clone(), commitment);

        info!("✅ Solana RPC client initialized: {}", rpc_url);

        Self {
            client,
            commitment,
            consecutive_failures: AtomicU32::new(0), // CYCLE-5: Initialize circuit breaker
        }
    }

    /// CYCLE-5 FIX: Check if circuit breaker is tripped
    /// Returns error if too many consecutive RPC failures have occurred
    pub fn check_circuit_breaker(&self) -> Result<()> {
        let failures = self.consecutive_failures.load(Ordering::Relaxed);

        if failures >= CIRCUIT_BREAKER_THRESHOLD {
            error!(
                "🚨 RPC CIRCUIT BREAKER TRIPPED: {} consecutive failures",
                failures
            );
            error!("   Trading halted to prevent losses during network issues");
            error!("   Manual intervention required - check RPC endpoint and restart bot");

            return Err(anyhow::anyhow!(
                "RPC circuit breaker tripped after {} consecutive failures. Manual restart required.",
                failures
            ));
        }

        Ok(())
    }

    /// CYCLE-5 FIX: Record successful RPC call (resets circuit breaker)
    fn record_success(&self) {
        let previous = self.consecutive_failures.swap(0, Ordering::Relaxed);
        if previous > 0 {
            info!(
                "✅ RPC recovered after {} failures - circuit breaker reset",
                previous
            );
        }
    }

    /// CYCLE-5 FIX: Record failed RPC call (increments circuit breaker counter)
    fn record_failure(&self) {
        let failures = self.consecutive_failures.fetch_add(1, Ordering::Relaxed) + 1;

        if failures >= CIRCUIT_BREAKER_THRESHOLD {
            error!(
                "🚨 RPC CIRCUIT BREAKER ABOUT TO TRIP: {}/{} failures",
                failures, CIRCUIT_BREAKER_THRESHOLD
            );
        } else if failures > 2 {
            warn!(
                "⚠️ RPC failures increasing: {}/{} (circuit breaker will trip at {})",
                failures, CIRCUIT_BREAKER_THRESHOLD, CIRCUIT_BREAKER_THRESHOLD
            );
        }
    }

    /// Get recent blockhash (needed for all transactions)
    /// HIGH-3 FIX: Added retry logic with exponential backoff
    /// CYCLE-5 FIX: Added circuit breaker tracking
    pub fn get_latest_blockhash(&self) -> Result<Hash> {
        debug!("Fetching latest blockhash...");

        // Retry up to 3 times with exponential backoff
        for attempt in 1..=3 {
            match self.client.get_latest_blockhash() {
                Ok(blockhash) => {
                    debug!("✅ Got blockhash: {}", blockhash);
                    self.record_success(); // CYCLE-5: Reset circuit breaker on success
                    return Ok(blockhash);
                }
                Err(e) => {
                    // Only retry on transient errors
                    let is_transient = e.to_string().contains("timeout")
                        || e.to_string().contains("network")
                        || e.to_string().contains("connection");

                    if !is_transient || attempt == 3 {
                        self.record_failure(); // CYCLE-5: Increment circuit breaker on failure
                        return Err(anyhow::anyhow!(
                            "Failed to fetch latest blockhash after {} attempts: {}",
                            attempt,
                            e
                        ));
                    }

                    // Exponential backoff: 100ms, 200ms, 400ms
                    let delay_ms = 100 * (1 << (attempt - 1));
                    warn!(
                        "⚠️ Blockhash fetch attempt {} failed, retrying in {}ms: {}",
                        attempt, delay_ms, e
                    );
                    std::thread::sleep(Duration::from_millis(delay_ms));
                }
            }
        }

        self.record_failure(); // CYCLE-5: Increment on final failure
        Err(anyhow::anyhow!(
            "Failed to fetch latest blockhash after retries"
        ))
    }

    /// Simulate transaction before sending (critical for safety)
    pub fn simulate_transaction(&self, transaction: &Transaction) -> Result<bool> {
        debug!(
            "Simulating transaction with {} instructions...",
            transaction.message.instructions.len()
        );

        let config = RpcSimulateTransactionConfig {
            sig_verify: false,
            commitment: Some(self.commitment),
            ..Default::default()
        };

        match self
            .client
            .simulate_transaction_with_config(transaction, config)
        {
            Ok(response) => {
                if let Some(err) = response.value.err {
                    warn!("❌ Transaction simulation failed: {:?}", err);

                    // Enhanced error analysis
                    if let Some(logs) = &response.value.logs {
                        warn!("📋 Failed transaction logs:");
                        for (i, log) in logs.iter().enumerate() {
                            if log.contains("Error")
                                || log.contains("failed")
                                || log.contains("insufficient")
                            {
                                warn!("   [{}] {}", i, log);
                            }
                        }

                        // Check for specific common errors
                        if logs.iter().any(|l| l.contains("insufficient funds")) {
                            warn!("   💰 INSUFFICIENT FUNDS - wallet needs more SOL or tokens");
                        }
                        if logs.iter().any(|l| l.contains("AccountNotFound")) {
                            warn!("   🔍 ACCOUNT NOT FOUND - likely missing ATA (Associated Token Account)");
                        }
                        if logs.iter().any(|l| l.contains("InvalidAccountData")) {
                            warn!("   ❌ INVALID ACCOUNT DATA - pool address might be wrong");
                        }
                        if logs.iter().any(|l| l.contains("slippage")) {
                            warn!("   📉 SLIPPAGE EXCEEDED - price moved too much");
                        }
                    }

                    return Ok(false);
                }

                if let Some(logs) = response.value.logs {
                    debug!("✅ Simulation successful. Log count: {}", logs.len());
                    // Only show logs if trace level enabled
                    if tracing::enabled!(tracing::Level::TRACE) {
                        for log_entry in &logs {
                            trace!("   {}", log_entry);
                        }
                    }
                }

                debug!("✅ Transaction simulation succeeded");
                Ok(true)
            }
            Err(e) => {
                warn!("❌ Failed to simulate transaction: {}", e);
                // Check for specific RPC errors
                let error_str = e.to_string();
                if error_str.contains("blockhash not found") {
                    warn!("   ⏰ Blockhash expired - need to get fresh blockhash");
                } else if error_str.contains("network") || error_str.contains("connection") {
                    warn!("   🌐 Network issue - RPC connection problem");
                }
                Ok(false)
            }
        }
    }

    /// Send transaction to blockchain
    pub fn send_transaction(&self, transaction: &Transaction) -> Result<Signature> {
        debug!("Sending transaction to blockchain...");

        let signature = self
            .client
            .send_transaction(transaction)
            .context("Failed to send transaction")?;

        info!("✅ Transaction sent: {}", signature);
        Ok(signature)
    }

    /// Get account data (for fetching pool state, token accounts, etc.)
    /// HIGH-3 FIX: Added retry logic with exponential backoff
    /// CYCLE-5 FIX: Added circuit breaker tracking
    pub fn get_account_data(&self, pubkey: &Pubkey) -> Result<Vec<u8>> {
        debug!("Fetching account data for: {}", pubkey);

        // Retry up to 3 times with exponential backoff
        for attempt in 1..=3 {
            match self.client.get_account(pubkey) {
                Ok(account) => {
                    debug!("✅ Got {} bytes of account data", account.data.len());
                    self.record_success(); // CYCLE-5: Reset circuit breaker on success
                    return Ok(account.data);
                }
                Err(e) => {
                    // Don't retry on "account not found" - that's permanent
                    let is_not_found = e.to_string().contains("AccountNotFound")
                        || e.to_string().contains("not found");

                    if is_not_found {
                        // Don't count "not found" as a failure - it's expected for invalid pools
                        return Err(anyhow::anyhow!("Account not found: {}", pubkey));
                    }

                    // Only retry on transient errors
                    let is_transient = e.to_string().contains("timeout")
                        || e.to_string().contains("network")
                        || e.to_string().contains("connection");

                    if !is_transient || attempt == 3 {
                        self.record_failure(); // CYCLE-5: Increment circuit breaker on failure
                        return Err(anyhow::anyhow!(
                            "Failed to fetch account {} after {} attempts: {}",
                            pubkey,
                            attempt,
                            e
                        ));
                    }

                    // Exponential backoff: 100ms, 200ms, 400ms
                    let delay_ms = 100 * (1 << (attempt - 1));
                    warn!(
                        "⚠️ Account fetch attempt {} failed, retrying in {}ms: {}",
                        attempt, delay_ms, e
                    );
                    std::thread::sleep(Duration::from_millis(delay_ms));
                }
            }
        }

        self.record_failure(); // CYCLE-5: Increment on final failure
        Err(anyhow::anyhow!(
            "Failed to fetch account data after retries"
        ))
    }

    /// Fetch multiple accounts in one RPC call (efficient)
    pub fn get_multiple_accounts(&self, pubkeys: &[Pubkey]) -> Result<Vec<Option<Vec<u8>>>> {
        debug!("Fetching {} accounts in batch...", pubkeys.len());

        let accounts = self
            .client
            .get_multiple_accounts(pubkeys)
            .context("Failed to fetch multiple accounts")?;

        let data: Vec<Option<Vec<u8>>> = accounts
            .into_iter()
            .map(|opt_account| opt_account.map(|acc| acc.data))
            .collect();

        let found = data.iter().filter(|d| d.is_some()).count();
        debug!("✅ Got {}/{} accounts", found, pubkeys.len());

        Ok(data)
    }

    /// Check if account exists AND has non-zero data (ghost pool protection)
    /// Returns false if account doesn't exist OR has 0 bytes of data
    pub fn account_exists(&self, pubkey: &Pubkey) -> Result<bool> {
        match self.client.get_account(pubkey) {
            Ok(account) => {
                // Account exists, but check if it has data
                if account.data.is_empty() || account.lamports == 0 {
                    debug!("⚠️ Account {} exists but is empty (ghost pool)", pubkey);
                    Ok(false)
                } else {
                    Ok(true)
                }
            }
            Err(e) => {
                // Check if it's "account not found" error vs other errors
                if e.to_string().contains("AccountNotFound") || e.to_string().contains("not found")
                {
                    Ok(false)
                } else {
                    Err(anyhow::anyhow!("Error checking account existence: {}", e))
                }
            }
        }
    }

    /// Get account owner (program that owns this account)
    pub fn get_account_owner(&self, pubkey: &Pubkey) -> Result<Pubkey> {
        let account = self
            .client
            .get_account(pubkey)
            .context(format!("Failed to fetch account {}", pubkey))?;

        Ok(account.owner)
    }

    /// Get transaction confirmation status
    /// Returns Ok(Some(true)) if confirmed successfully, Ok(Some(false)) if failed, Ok(None) if pending
    pub fn get_transaction_status(&self, signature: &Signature) -> Result<Option<bool>> {
        // Poll blockchain for transaction status
        match self.client.get_signature_status(signature) {
            Ok(Some(result)) => {
                // Transaction found in blockchain
                match result {
                    Ok(_) => Ok(Some(true)),   // Confirmed successfully
                    Err(_) => Ok(Some(false)), // Failed on-chain
                }
            }
            Ok(None) => Ok(None), // Not yet confirmed
            Err(e) => Err(anyhow::anyhow!("Error checking transaction status: {}", e)),
        }
    }

    /// Get balance of an account (in lamports)
    pub fn get_balance(&self, pubkey: &Pubkey) -> Result<u64> {
        let balance = self
            .client
            .get_balance(pubkey)
            .context(format!("Failed to get balance for {}", pubkey))?;

        Ok(balance)
    }

    /// Health check - verify RPC connection is working
    pub fn health_check(&self) -> Result<bool> {
        match self.client.get_health() {
            Ok(_) => {
                debug!("✅ RPC health check passed");
                Ok(true)
            }
            Err(e) => {
                warn!("❌ RPC health check failed: {}", e);
                Ok(false)
            }
        }
    }

    /// Get current slot
    pub fn get_slot(&self) -> Result<u64> {
        let slot = self
            .client
            .get_slot()
            .context("Failed to get current slot")?;

        Ok(slot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rpc_client_creation() {
        let rpc_url = "https://api.mainnet-beta.solana.com".to_string();
        let client = SolanaRpcClient::new(rpc_url);

        // Just test that it creates without panicking
        assert!(client.commitment.is_confirmed());
    }

    // Note: Most tests require a live RPC connection and are better suited for integration tests
}
