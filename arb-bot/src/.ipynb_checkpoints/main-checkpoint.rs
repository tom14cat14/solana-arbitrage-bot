use anyhow::Result;
use tracing::{info, error};
use tokio::signal;

mod arbitrage_engine;
mod shredstream_client;  // NEW: REST API client for ShredStream service
mod production_features;
mod config;
mod jupiter;
mod dex_registry;
mod metrics;
mod safety_systems;
mod secure_wallet;
use arbitrage_engine::ArbitrageEngine;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("💰 Starting Advanced Arbitrage Bot (Cross-DEX Trading)");
    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // Load configuration from environment
    let config = match config::Config::from_env() {
        Ok(config) => config,
        Err(e) => {
            error!("❌ Failed to load configuration: {}", e);
            return Err(e);
        }
    };

    info!("✅ Configuration loaded:");
    info!("  • ShredStream: {}", config.shreds_endpoint);
    info!("  • Jupiter API: {}***", &config.jupiter_api_key[..8]);
    info!("  • JITO Endpoint: {}", config.jito_endpoint);

    // Use configuration from config struct
    let min_profit_sol = config.min_profit_sol;
    let max_position_size_sol = config.max_position_size_sol;
    let enable_real_trading = config.enable_real_trading;
    let paper_trading = config.paper_trading;

    info!("🔧 Production Arbitrage Configuration:");
    info!("  • Strategy: Cross-DEX price differences across 15+ DEXs");
    info!("  • Min profit threshold: {:.4} SOL", min_profit_sol);
    info!("  • Max position size: {:.1} SOL", max_position_size_sol);
    info!("  • Trading mode: {} (Real: {}, Paper: {})",
          if enable_real_trading { "LIVE" } else { "PAPER" },
          enable_real_trading, paper_trading);
    info!("  • Risk profile: Conservative with MEV protection");

    // Create production arbitrage engine with Phase 4 features
    info!("🚀 Initializing production arbitrage engine with advanced features...");
    let mut arbitrage_engine = match ArbitrageEngine::new(
        config.shreds_endpoint,
        config.jupiter_api_key,
        config.jito_endpoint,
        min_profit_sol,
        max_position_size_sol,
        enable_real_trading,
        paper_trading,
    ).await {
        Ok(engine) => {
            info!("✅ Advanced arbitrage engine initialized successfully");
            engine
        }
        Err(e) => {
            error!("❌ Failed to initialize arbitrage engine: {}", e);
            return Err(e);
        }
    };

    // Display initial statistics
    let stats = arbitrage_engine.get_stats();
    info!("📊 Initial Arbitrage Engine Status:");
    info!("  • Price updates processed: {}", stats.price_updates_processed);
    info!("  • Opportunities detected: {}", stats.opportunities_detected);
    info!("  • Total profit: {:.6} SOL", stats.total_profit_sol);

    // Show supported DEX pairs for arbitrage
    info!("💱 Advanced Arbitrage Trading Infrastructure:");
    info!("  • Supported DEXs: 15+ (Raydium, Orca, Jupiter, Meteora, Serum, etc.)");
    info!("  • Trading pairs: 100+ cross-DEX opportunities");
    info!("  • Min profit threshold: {:.4} SOL", min_profit_sol);
    info!("  • Real-time price monitoring with opportunity detection");

    // Set up graceful shutdown handler
    let shutdown_handle = tokio::spawn(async {
        match signal::ctrl_c().await {
            Ok(()) => {
                info!("🛑 Shutdown signal received");
            }
            Err(err) => {
                error!("❌ Failed to listen for shutdown signal: {}", err);
            }
        }
    });

    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    info!("🚀 Advanced Arbitrage Engine is now LIVE - Real-time cross-DEX scanning...");
    info!("💡 Press Ctrl+C to stop");
    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // Start advanced arbitrage monitoring (this runs until shutdown)
    tokio::select! {
        result = arbitrage_engine.start_monitoring() => {
            match result {
                Ok(()) => info!("✅ Advanced arbitrage monitoring completed successfully"),
                Err(e) => {
                    error!("❌ Advanced arbitrage monitoring failed: {}", e);
                    return Err(e);
                }
            }
        }
        _ = shutdown_handle => {
            info!("🛑 Graceful shutdown initiated");
        }
    }

    // Final statistics before shutdown
    let final_stats = arbitrage_engine.get_stats();
    info!("📊 Final Advanced Arbitrage Statistics:");
    info!("  • Runtime: {} seconds", final_stats.uptime_seconds);
    info!("  • Price updates processed: {}", final_stats.price_updates_processed);
    info!("  • Opportunities detected: {}", final_stats.opportunities_detected);
    info!("  • Opportunities executed: {}", final_stats.opportunities_executed);
    info!("  • Cross-DEX trades: {}", final_stats.cross_dex_opportunities);
    info!("  • Failed executions: {}", final_stats.failed_executions);
    info!("  • Total profit: {:.6} SOL", final_stats.total_profit_sol);

    let success_rate = if final_stats.opportunities_detected > 0 {
        (final_stats.opportunities_executed as f64 / final_stats.opportunities_detected as f64) * 100.0
    } else {
        0.0
    };
    info!("  • Success rate: {:.1}%", success_rate);

    // Generate final performance report
    if let Ok(report) = arbitrage_engine.generate_performance_report(24).await {
        info!("📋 24-Hour Advanced Arbitrage Performance:");
        info!("  • Total opportunities: {}", report.total_opportunities);
        info!("  • Total executions: {}", report.total_executions);
        info!("  • Total profit: {:.6} SOL", report.total_profit_sol);
        info!("  • Average execution time: {:.1}ms", report.average_execution_time_ms);
        info!("  • Success rate: {:.1}%", report.success_rate_percent);

        // Show profit breakdown by engine
        for (engine, profit) in &report.profit_by_engine {
            if *profit > 0.0 {
                info!("  • {} profit: {:.6} SOL", engine, profit);
            }
        }
    }

    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    info!("👋 Advanced Arbitrage Bot shutdown complete");

    Ok(())
}