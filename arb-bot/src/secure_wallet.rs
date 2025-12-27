use anyhow::Result;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::pubkey::Pubkey;
use solana_rpc_client::rpc_client::RpcClient;
use std::sync::Arc;
use std::path::Path;
use std::fs;
use tracing::{info, warn};
use serde::{Deserialize, Serialize};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use aes_gcm::{aead::Aead, KeyInit};
use pbkdf2::pbkdf2;
use pbkdf2::hmac::Hmac;
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletConfig {
    pub main_private_key: String,
    pub hot_private_key: Option<String>,
    pub cold_wallet_address: Option<Pubkey>,
    pub min_balance_sol: Option<f64>,
    pub description: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Enterprise-grade wallet management for production trading
pub struct SecureWalletManager {
    main_keypair: Arc<Keypair>,
    hot_keypair: Option<Arc<Keypair>>,
    cold_wallet_address: Option<Pubkey>,
    rpc_client: RpcClient,
    min_balance_sol: f64,
    encrypted_storage: bool,
}

impl std::fmt::Debug for SecureWalletManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecureWalletManager")
            .field("main_keypair", &self.main_keypair.pubkey())
            .field("hot_keypair", &self.hot_keypair.as_ref().map(|k| k.pubkey()))
            .field("cold_wallet_address", &self.cold_wallet_address)
            .field("min_balance_sol", &self.min_balance_sol)
            .field("encrypted_storage", &self.encrypted_storage)
            .finish()
    }
}

impl SecureWalletManager {
    /// Create new wallet manager with secure key loading
    pub async fn new(rpc_endpoint: &str) -> Result<Self> {
        let rpc_client = RpcClient::new(rpc_endpoint.to_string());

        // Try encrypted file first (RECOMMENDED)
        if let Ok(key_file_path) = std::env::var("WALLET_KEY_FILE_PATH") {
            let password_env = std::env::var("WALLET_PASSWORD_ENV_VAR")
                .unwrap_or_else(|_| "WALLET_PASSWORD".to_string());
            let password = std::env::var(&password_env)
                .map_err(|_| anyhow::anyhow!("Wallet password not found in environment variable: {}", password_env))?;

            return Self::from_encrypted_file(&key_file_path, &password, rpc_client).await;
        }

        // Fallback to environment variable (LESS SECURE)
        if let Ok(private_key_b58) = std::env::var("WALLET_PRIVATE_KEY") {
            warn!("⚠️ Loading private key from environment variable - not recommended for production");
            return Self::from_environment_variable(&private_key_b58, rpc_client).await;
        }

        Err(anyhow::anyhow!("No wallet configuration found. Set WALLET_KEY_FILE_PATH or WALLET_PRIVATE_KEY"))
    }

    /// Load wallet from encrypted file (RECOMMENDED)
    async fn from_encrypted_file(file_path: &str, password: &str, rpc_client: RpcClient) -> Result<Self> {
        info!("🔐 Loading encrypted wallet from: {}", file_path);

        if !Path::new(file_path).exists() {
            return Err(anyhow::anyhow!("Encrypted wallet file not found: {}", file_path));
        }

        let encrypted_data = fs::read(file_path)
            .map_err(|e| anyhow::anyhow!("Failed to read encrypted wallet file: {}", e))?;

        let wallet_data = Self::decrypt_wallet_data(&encrypted_data, password)?;
        let wallet_config: WalletConfig = serde_json::from_str(&wallet_data)
            .map_err(|e| anyhow::anyhow!("Failed to parse wallet configuration: {}", e))?;

        let main_keypair = Self::parse_private_key(&wallet_config.main_private_key)?;
        let hot_keypair = wallet_config.hot_private_key
            .map(|key| Self::parse_private_key(&key))
            .transpose()?
            .map(Arc::new);

        info!("✅ Encrypted wallet loaded successfully");
        info!("  • Main wallet: {}", main_keypair.pubkey());
        if let Some(ref hot) = hot_keypair {
            info!("  • Hot wallet: {}", hot.pubkey());
        }
        if let Some(cold) = wallet_config.cold_wallet_address {
            info!("  • Cold wallet: {}", cold);
        }

        Ok(Self {
            main_keypair: Arc::new(main_keypair),
            hot_keypair,
            cold_wallet_address: wallet_config.cold_wallet_address,
            rpc_client,
            min_balance_sol: wallet_config.min_balance_sol.unwrap_or(0.1),
            encrypted_storage: true,
        })
    }

    /// Load wallet from environment variable (less secure)
    async fn from_environment_variable(private_key_b58: &str, rpc_client: RpcClient) -> Result<Self> {
        let main_keypair = Self::parse_private_key(private_key_b58)?;

        // Optional hot wallet
        let hot_keypair = if let Ok(hot_key_b58) = std::env::var("HOT_WALLET_PRIVATE_KEY") {
            Some(Arc::new(Self::parse_private_key(&hot_key_b58)?))
        } else {
            None
        };

        // Optional cold wallet
        let cold_wallet_address = std::env::var("COLD_WALLET_ADDRESS")
            .ok()
            .and_then(|addr| addr.parse().ok());

        let min_balance_sol = std::env::var("MIN_WALLET_BALANCE_SOL")
            .unwrap_or_else(|_| "0.1".to_string())
            .parse()
            .unwrap_or(0.1);

        info!("✅ Wallet manager initialized from environment");
        info!("  • Main wallet: {}", main_keypair.pubkey());
        if let Some(ref hot) = hot_keypair {
            info!("  • Hot wallet: {}", hot.pubkey());
        }
        if let Some(cold) = cold_wallet_address {
            info!("  • Cold wallet: {}", cold);
        }

        Ok(Self {
            main_keypair: Arc::new(main_keypair),
            hot_keypair,
            cold_wallet_address,
            rpc_client,
            min_balance_sol,
            encrypted_storage: false,
        })
    }

    /// Comprehensive wallet security verification for live trading
    pub async fn is_wallet_secure(&self) -> bool {
        // 1. Check wallet balance
        if let Err(e) = self.verify_sufficient_balance().await {
            warn!("❌ Wallet balance check failed: {}", e);
            return false;
        }

        // 2. Verify network connectivity
        if let Err(e) = self.verify_network_connectivity().await {
            warn!("❌ Network connectivity check failed: {}", e);
            return false;
        }

        // 3. Test private key access
        if let Err(e) = self.verify_key_access() {
            warn!("❌ Private key access check failed: {}", e);
            return false;
        }

        // 4. Check account status
        if let Err(e) = self.verify_account_status().await {
            warn!("❌ Account status check failed: {}", e);
            return false;
        }

        info!("✅ All wallet security checks passed");
        true
    }

    /// Get SOL balance for main wallet
    pub async fn get_sol_balance(&self) -> Result<f64> {
        let balance_lamports = self.rpc_client
            .get_balance(&self.main_keypair.pubkey())
            .map_err(|e| anyhow::anyhow!("Failed to get wallet balance: {}", e))?;

        Ok(balance_lamports as f64 / 1_000_000_000.0)
    }

    /// Get main trading keypair
    pub fn get_main_keypair(&self) -> Arc<Keypair> {
        Arc::clone(&self.main_keypair)
    }

    /// Get hot wallet keypair (if configured)
    pub fn get_hot_keypair(&self) -> Option<Arc<Keypair>> {
        self.hot_keypair.as_ref().map(Arc::clone)
    }

    /// Get cold wallet address (if configured)
    pub fn get_cold_wallet(&self) -> Option<Pubkey> {
        self.cold_wallet_address
    }

    /// Get RPC client
    pub fn get_rpc_client(&self) -> &RpcClient {
        &self.rpc_client
    }

    /// Verify wallet has sufficient balance for trading
    async fn verify_sufficient_balance(&self) -> Result<()> {
        let balance = self.get_sol_balance().await?;

        if balance < self.min_balance_sol {
            return Err(anyhow::anyhow!(
                "Insufficient balance: {:.6} SOL < {:.6} SOL minimum",
                balance, self.min_balance_sol
            ));
        }

        info!("✅ Wallet balance: {:.6} SOL (minimum: {:.6} SOL)", balance, self.min_balance_sol);
        Ok(())
    }

    /// Verify network connectivity and RPC health
    async fn verify_network_connectivity(&self) -> Result<()> {
        // Test basic RPC connectivity
        let _health = self.rpc_client.get_health()
            .map_err(|e| anyhow::anyhow!("RPC health check failed: {}", e))?;

        // Get recent blockhash to verify network access
        let _blockhash = self.rpc_client.get_latest_blockhash()
            .map_err(|e| anyhow::anyhow!("Failed to get recent blockhash: {}", e))?;

        info!("✅ Network connectivity verified");
        Ok(())
    }

    /// Verify private key access by signing test message
    fn verify_key_access(&self) -> Result<()> {
        let test_message = b"wallet_security_check";
        let _signature = self.main_keypair.try_sign_message(test_message)
            .map_err(|e| anyhow::anyhow!("Failed to sign test message: {}", e))?;

        info!("✅ Private key access verified");
        Ok(())
    }

    /// Verify account status and permissions
    async fn verify_account_status(&self) -> Result<()> {
        let account_info = self.rpc_client
            .get_account(&self.main_keypair.pubkey())
            .map_err(|e| anyhow::anyhow!("Failed to get account info: {}", e))?;

        if account_info.lamports == 0 {
            return Err(anyhow::anyhow!("Account has zero balance"));
        }

        info!("✅ Account status verified");
        Ok(())
    }

    /// Parse private key from base58 string
    fn parse_private_key(private_key_str: &str) -> Result<Keypair> {
        let keypair = Keypair::from_base58_string(private_key_str);
        Ok(keypair)
    }

    /// Decrypt wallet data using AES-256-GCM
    fn decrypt_wallet_data(encrypted_data: &[u8], password: &str) -> Result<String> {
        if encrypted_data.len() < 32 {
            return Err(anyhow::anyhow!("Invalid encrypted data format"));
        }

        let salt = &encrypted_data[0..16];
        let nonce = &encrypted_data[16..28];
        let ciphertext = &encrypted_data[28..];

        // Derive key from password
        let mut key = [0u8; 32];
        pbkdf2::<HmacSha256>(password.as_bytes(), salt, 10000, &mut key);

        let cipher = Aes256Gcm::new(aes_gcm::Key::<Aes256Gcm>::from_slice(&key));
        let nonce = Nonce::from_slice(nonce);

        let plaintext = cipher.decrypt(nonce, ciphertext)
            .map_err(|_| anyhow::anyhow!("Failed to decrypt wallet data - incorrect password?"))?;

        String::from_utf8(plaintext)
            .map_err(|e| anyhow::anyhow!("Invalid UTF-8 in decrypted data: {}", e))
    }

    /// Create encrypted wallet file (for initial setup)
    pub fn create_encrypted_wallet_file(file_path: &str, password: &str, config: &WalletConfig) -> Result<()> {
        let config_json = serde_json::to_string_pretty(config)
            .map_err(|e| anyhow::anyhow!("Failed to serialize wallet config: {}", e))?;

        let encrypted_data = Self::encrypt_wallet_data(&config_json, password)?;

        fs::write(file_path, encrypted_data)
            .map_err(|e| anyhow::anyhow!("Failed to write encrypted wallet file: {}", e))?;

        info!("✅ Encrypted wallet file created: {}", file_path);
        Ok(())
    }

    /// Encrypt wallet data using AES-256-GCM
    fn encrypt_wallet_data(data: &str, password: &str) -> Result<Vec<u8>> {
        use rand::RngCore;

        let mut salt = [0u8; 16];
        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut salt);
        rand::thread_rng().fill_bytes(&mut nonce_bytes);

        // Derive key from password
        let mut key = [0u8; 32];
        pbkdf2::<HmacSha256>(password.as_bytes(), &salt, 10000, &mut key);

        let cipher = Aes256Gcm::new(aes_gcm::Key::<Aes256Gcm>::from_slice(&key));
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher.encrypt(nonce, data.as_bytes())
            .map_err(|_| anyhow::anyhow!("Failed to encrypt wallet data"))?;

        // Combine salt + nonce + ciphertext
        let mut result = Vec::new();
        result.extend_from_slice(&salt);
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);

        Ok(result)
    }
}

/// Utility function to create a new wallet configuration
pub fn create_new_wallet_config(description: &str) -> WalletConfig {
    let main_keypair = Keypair::new();
    let hot_keypair = Keypair::new();

    WalletConfig {
        main_private_key: main_keypair.to_base58_string(),
        hot_private_key: Some(hot_keypair.to_base58_string()),
        cold_wallet_address: None, // To be set manually for security
        min_balance_sol: Some(0.1),
        description: description.to_string(),
        created_at: chrono::Utc::now(),
    }
}