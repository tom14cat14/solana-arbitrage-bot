use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    // Network endpoints
    pub shreds_endpoint: String,
    pub jupiter_endpoint: String,
    pub jito_endpoint: String,
    pub rpc_endpoint: String,

    // API keys
    pub jupiter_api_key: String,
    pub x_token: String,  // ERPC gRPC authentication token

    // Trading configuration
    pub min_profit_sol: f64,
    pub max_position_size_sol: f64,
    pub enable_real_trading: bool,
    pub paper_trading: bool,

    // Risk management
    pub max_daily_loss_sol: f64,
    pub max_daily_trades: u32,
    pub max_consecutive_failures: u32,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Ok(Config {
            // Network endpoints
            shreds_endpoint: env::var("SHREDSTREAM_ENDPOINT")
                .unwrap_or_else(|_| "grpc-ny6-1.erpc.global:443".to_string()),
            jupiter_endpoint: env::var("JUPITER_ENDPOINT")
                .unwrap_or_else(|_| "https://quote-api.jup.ag/v6".to_string()),
            jito_endpoint: env::var("JITO_ENDPOINT")
                .unwrap_or_else(|_| "https://mainnet.block-engine.jito.wtf".to_string()),
            rpc_endpoint: env::var("SOLANA_RPC_ENDPOINT")
                .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string()),

            // API keys
            jupiter_api_key: env::var("JUPITER_API_KEY")
                .unwrap_or_else(|_| "test_key_for_demo".to_string()),
            x_token: env::var("X_TOKEN")
                .unwrap_or_else(|_| "507c3fff-6dc7-4d6d-8915-596be560814f".to_string()),

            // Trading configuration
            min_profit_sol: env::var("MIN_PROFIT_SOL")
                .unwrap_or_else(|_| "0.005".to_string())
                .parse()
                .unwrap_or(0.005),
            max_position_size_sol: env::var("MAX_POSITION_SIZE_SOL")
                .unwrap_or_else(|_| "0.5".to_string())
                .parse()
                .unwrap_or(0.5),
            enable_real_trading: env::var("ENABLE_REAL_TRADING")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(false),
            paper_trading: env::var("PAPER_TRADING")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),

            // Risk management
            max_daily_loss_sol: env::var("MAX_DAILY_LOSS_SOL")
                .unwrap_or_else(|_| "1.0".to_string())
                .parse()
                .unwrap_or(1.0),
            max_daily_trades: env::var("MAX_DAILY_TRADES")
                .unwrap_or_else(|_| "200".to_string())
                .parse()
                .unwrap_or(200),
            max_consecutive_failures: env::var("MAX_CONSECUTIVE_FAILURES")
                .unwrap_or_else(|_| "5".to_string())
                .parse()
                .unwrap_or(5),
        })
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::from_env().unwrap_or(Config {
            shreds_endpoint: "grpc-ny6-1.erpc.global:443".to_string(),
            jupiter_endpoint: "https://quote-api.jup.ag/v6".to_string(),
            jito_endpoint: "https://mainnet.block-engine.jito.wtf".to_string(),
            rpc_endpoint: "https://api.mainnet-beta.solana.com".to_string(),
            jupiter_api_key: "test_key_for_demo".to_string(),
            x_token: "507c3fff-6dc7-4d6d-8915-596be560814f".to_string(),
            min_profit_sol: 0.005,
            max_position_size_sol: 0.5,
            enable_real_trading: false,
            paper_trading: true,
            max_daily_loss_sol: 1.0,
            max_daily_trades: 200,
            max_consecutive_failures: 5,
        })
    }
}