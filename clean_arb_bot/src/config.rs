use anyhow::Result;
use std::env;

/// Configuration for the arbitrage bot
#[derive(Debug, Clone)]
pub struct Config {
    pub shredstream_url: String,
    pub solana_rpc_url: Option<String>,
    pub capital_sol: f64,
    pub max_position_size_sol: f64,
    pub min_profit_margin_multiplier: f64,  // Replaced min_profit_sol with margin multiplier
    pub min_spread_percentage: f64,
    pub max_daily_trades: u64,
    pub daily_loss_limit_sol: f64,
    pub max_consecutive_failures: u64,
    pub enable_real_trading: bool,
    pub paper_trading: bool,
    pub wallet_private_key: Option<String>,
    pub jupiter_api_key: Option<String>,
}

impl Config {
    /// Calculate JITO tip based on profit (3-10% of profit, capped at 0.001 SOL)
    pub fn calculate_jito_tip(&self, gross_profit_sol: f64) -> f64 {
        if gross_profit_sol < 0.1 {
            // Small profits: 3-5% tip
            let tip = gross_profit_sol * 0.03;
            tip.min(0.001).max(0.0001)  // Cap at 0.001 SOL, min 0.0001 SOL
        } else if gross_profit_sol < 1.0 {
            // Medium profits: 5-7% tip
            let tip = gross_profit_sol * 0.05;
            tip.min(0.001).max(0.0001)
        } else {
            // Large profits: 7-10% tip
            let tip = gross_profit_sol * 0.07;
            tip.min(0.001).max(0.0001)
        }
    }

    /// Calculate total fees for a trade (JITO tip + gas + compute)
    pub fn calculate_total_fees(&self, gross_profit_sol: f64) -> f64 {
        let jito_tip = self.calculate_jito_tip(gross_profit_sol);
        let gas_fee = 0.00005;  // ~50,000 lamports typical
        let compute_fee = 0.00001;  // ~10,000 lamports typical
        jito_tip + gas_fee + compute_fee
    }

    /// Calculate minimum acceptable profit based on fees and margin
    /// NEW (2025-10-11): Net profit must be > total_fees + 0.5% of gross profit
    /// This ensures we beat fees AND have a small safety margin
    pub fn calculate_min_acceptable_profit(&self, gross_profit_sol: f64) -> f64 {
        let total_fees = self.calculate_total_fees(gross_profit_sol);
        let margin = 0.005 * gross_profit_sol;  // 0.5% of gross profit
        total_fees + margin
    }

    /// Check if a trade is profitable after fees with required margin
    /// UPDATED (2025-10-11): User requirement - beat fees + 0.5% gross profit margin
    /// OLD: net_profit >= fees * 1.2 (20% margin)
    /// NEW: net_profit >= fees + 0.5% of gross (realistic arbitrage)
    pub fn is_profitable_after_fees(&self, gross_profit_sol: f64) -> bool {
        let total_fees = self.calculate_total_fees(gross_profit_sol);
        let net_profit = gross_profit_sol - total_fees;
        let required_margin = 0.005 * gross_profit_sol;  // 0.5% of gross as safety margin
        net_profit >= (total_fees + required_margin)
    }
}

impl Config {
    pub fn from_env() -> Result<Self> {
        // Load .env file
        dotenvy::dotenv().ok();

        let config = Self {
            shredstream_url: env::var("SHREDSTREAM_SERVICE_URL")
                .unwrap_or_else(|_| "http://localhost:8080".to_string()),

            solana_rpc_url: env::var("SOLANA_RPC_URL").ok(),

            capital_sol: env::var("CAPITAL_SOL")
                .unwrap_or_else(|_| "2.0".to_string())
                .parse()?,

            max_position_size_sol: env::var("MAX_POSITION_SIZE_SOL")
                .unwrap_or_else(|_| "0.5".to_string())
                .parse()?,

            min_profit_margin_multiplier: env::var("MIN_PROFIT_MARGIN_MULTIPLIER")
                .unwrap_or_else(|_| "2.0".to_string())  // Default: 2x fees (100% margin)
                .parse()?,

            min_spread_percentage: env::var("MIN_SPREAD_PERCENTAGE")
                .unwrap_or_else(|_| "0.3".to_string())  // HIGH FIX: 0.3% - realistic for cross-DEX arbitrage
                .parse()?,

            max_daily_trades: env::var("MAX_DAILY_TRADES")
                .unwrap_or_else(|_| "200".to_string())
                .parse()?,

            daily_loss_limit_sol: env::var("DAILY_LOSS_LIMIT_SOL")
                .unwrap_or_else(|_| "0.5".to_string())
                .parse()?,

            max_consecutive_failures: env::var("MAX_CONSECUTIVE_FAILURES")
                .unwrap_or_else(|_| "100".to_string())  // Increased for market chaos - keep running!
                .parse()?,

            enable_real_trading: env::var("ENABLE_REAL_TRADING")
                .unwrap_or_else(|_| "false".to_string())
                .to_lowercase() == "true",

            paper_trading: env::var("PAPER_TRADING")
                .unwrap_or_else(|_| "true".to_string())
                .to_lowercase() == "true",

            wallet_private_key: env::var("WALLET_PRIVATE_KEY").ok(),

            jupiter_api_key: env::var("JUPITER_API_KEY").ok(),
        };

        // MEDIUM FIX: Validate config parameters
        config.validate()?;

        Ok(config)
    }

    /// Validate configuration parameters
    /// MEDIUM FIX: Ensure all config values are sensible
    fn validate(&self) -> Result<()> {
        // Validate capital is positive
        if self.capital_sol <= 0.0 {
            return Err(anyhow::anyhow!(
                "Invalid capital_sol: {} (must be > 0)",
                self.capital_sol
            ));
        }

        // Validate max position size doesn't exceed capital
        if self.max_position_size_sol > self.capital_sol {
            return Err(anyhow::anyhow!(
                "Invalid max_position_size_sol: {} exceeds capital_sol: {}",
                self.max_position_size_sol,
                self.capital_sol
            ));
        }

        // Validate profit margin multiplier is reasonable
        if self.min_profit_margin_multiplier < 1.0 {
            return Err(anyhow::anyhow!(
                "Invalid min_profit_margin_multiplier: {} (must be >= 1.0 for positive margin)",
                self.min_profit_margin_multiplier
            ));
        }
        if self.min_profit_margin_multiplier > 10.0 {
            return Err(anyhow::anyhow!(
                "Invalid min_profit_margin_multiplier: {} (> 10.0 is too conservative, bot won't find trades)",
                self.min_profit_margin_multiplier
            ));
        }

        // Validate min spread (allow 0 for dynamic calculation)
        // NOTE: min_spread_percentage is DEPRECATED - now calculated dynamically
        // Keeping field for backward compatibility, but 0 is allowed
        if self.min_spread_percentage < 0.0 {
            return Err(anyhow::anyhow!(
                "Invalid min_spread_percentage: {} (must be >= 0, or 0 for dynamic)",
                self.min_spread_percentage
            ));
        }

        // Validate max daily trades is reasonable
        if self.max_daily_trades == 0 {
            return Err(anyhow::anyhow!(
                "Invalid max_daily_trades: 0 (bot would do nothing)"
            ));
        }

        // Validate all float values are finite
        if !self.capital_sol.is_finite() {
            return Err(anyhow::anyhow!("capital_sol must be finite"));
        }
        if !self.max_position_size_sol.is_finite() {
            return Err(anyhow::anyhow!("max_position_size_sol must be finite"));
        }
        if !self.min_profit_margin_multiplier.is_finite() {
            return Err(anyhow::anyhow!("min_profit_margin_multiplier must be finite"));
        }
        if !self.min_spread_percentage.is_finite() {
            return Err(anyhow::anyhow!("min_spread_percentage must be finite"));
        }
        if !self.daily_loss_limit_sol.is_finite() {
            return Err(anyhow::anyhow!("daily_loss_limit_sol must be finite"));
        }

        Ok(())
    }
}
