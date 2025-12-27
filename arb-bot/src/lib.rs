// Core modules
pub mod arbitrage_engine;
pub mod config;
pub mod dex_registry;
pub mod shredstream_udp; // Real ERPC ShredStream UDP listener (port 20000)
pub mod shredstream_price_monitor;
// pub mod shredstream_client; // gRPC implementation - not used for IP-whitelisted ShredStream
// pub mod real_shredstream; // Old UDP implementation - not used
// pub mod real_shredstream_client; // Old implementation
pub mod real_price_feed;
pub mod production_features;
pub mod jupiter;
pub mod protobuf_processor;
pub mod dex_transaction_parser;
pub mod dex_instruction_builder;
pub mod metrics;
pub mod safety_systems;
pub mod secure_wallet;

// Re-exports for easier usage
pub use arbitrage_engine::ArbitrageEngine;
pub use config::Config;
pub use dex_registry::DexRegistry;
pub use safety_systems::SafetySystem;
pub use metrics::MetricsCollector;

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    #[tokio::test]
    async fn test_arbitrage_engine_initialization() -> Result<()> {
        let engine = ArbitrageEngine::new(
            "https://shreds-ny6-1.erpc.global".to_string(),
            "test_key".to_string(),
            "https://mainnet.block-engine.jito.wtf".to_string(),
            0.005,
            0.5,
            false,
            true,
        ).await?;

        // Test basic initialization
        let stats = engine.get_stats();
        assert_eq!(stats.opportunities_detected, 0);
        assert_eq!(stats.opportunities_executed, 0);

        Ok(())
    }

    #[test]
    fn test_dex_registry_initialization() {
        let registry = DexRegistry::new();

        // Should have multiple DEXs loaded
        assert!(!registry.dexs.is_empty());

        // Should contain major DEXs
        assert!(registry.dexs.contains_key("Raydium_AMM_V4"));
        assert!(registry.dexs.contains_key("Orca_Whirlpools"));
        assert!(registry.dexs.contains_key("Jupiter"));
    }

    #[test]
    fn test_safety_system_initialization() {
        let safety_system = SafetySystem::new();
        let status = safety_system.get_safety_status();

        // Should start in safe state
        assert!(status.trading_allowed);
        assert!(!status.emergency_active);
        assert!(!status.main_breaker_active);
        assert_eq!(status.active_positions, 0);
        assert_eq!(status.trades_today, 0);
    }

    #[test]
    fn test_metrics_collector_initialization() {
        let metrics = MetricsCollector::new();
        let stats = metrics.get_metrics();

        // Should start with zero metrics
        assert_eq!(stats.arbitrage_metrics.opportunities_detected, 0);
        assert_eq!(stats.arbitrage_metrics.opportunities_executed, 0);
        assert_eq!(stats.arbitrage_metrics.opportunities_failed, 0);
    }

    #[tokio::test]
    async fn test_config_loading() -> Result<()> {
        // Test loading config from environment
        std::env::set_var("SHREDS_ENDPOINT", "https://shreds-ny6-1.erpc.global");
        std::env::set_var("JUPITER_API_KEY", "test_key");

        let config = Config::from_env()?;

        assert_eq!(config.shreds_endpoint, "https://shreds-ny6-1.erpc.global");
        assert_eq!(config.jupiter_api_key, "test_key");

        Ok(())
    }
}