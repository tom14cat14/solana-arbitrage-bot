use anyhow::Result;
use shared_bot_infrastructure::*;
use tracing::{info, warn, error};
use tokio::signal;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("💰 Starting Arbitrage Bot (Cross-DEX Trading)");
    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // Load configuration from environment
    let config = match SharedConfig::from_env() {
        Ok(config) => config,
        Err(e) => {
            error!("❌ Failed to load configuration: {}", e);
            return Err(e);
        }
    };

    info!("✅ Configuration loaded:");
    info!("  • ShredStream: {}", config.shreds_endpoint);
    info!("  • Jupiter API: {}***", &config.jupiter_api_key[..8]);

    // Arbitrage Bot Configuration - Optimized for cross-DEX opportunities
    let arb_config = MonitorConfig {
        enable_sandwich_attacks: false,  // DISABLED: Handled by separate MEV bot
        enable_arbitrage: true,          // PRIMARY: Cross-DEX arbitrage
        enable_liquidations: false,      // DISABLED: Handled by MEV bot
        max_concurrent_opportunities: 15, // Higher concurrency for arb (lower risk)
        opportunity_timeout_ms: 3000,    // Longer timeout (arb can be slightly slower)
        stats_reporting_interval_ms: 30000, // 30 second reports
    };

    info!("🔧 Arbitrage Bot Configuration:");
    info!("  • Sandwich attacks: ❌ DISABLED (separate MEV bot)");
    info!("  • Arbitrage: ✅ ENABLED");
    info!("  • Liquidations: ❌ DISABLED (separate MEV bot)");
    info!("  • Max concurrent: {}", arb_config.max_concurrent_opportunities);
    info!("  • Opportunity timeout: {}ms", arb_config.opportunity_timeout_ms);
    info!("  • Strategy: Cross-DEX price differences");
    info!("  • Risk profile: Lower risk, consistent profits");

    // Create arbitrage monitor with optimized settings
    info!("🚀 Initializing arbitrage monitoring infrastructure...");
    let mut arb_monitor = match MempoolMonitor::new(
        config.shreds_endpoint,
        config.jupiter_api_key,
        "https://mainnet.jito.wtf".to_string(), // Production Jito endpoint
        arb_config,
    ).await {
        Ok(monitor) => {
            info!("✅ Arbitrage monitor initialized successfully");
            monitor
        }
        Err(e) => {
            error!("❌ Failed to initialize arbitrage monitor: {}", e);
            return Err(e);
        }
    };

    // Display initial statistics
    let stats = arb_monitor.get_stats();
    info!("📊 Initial Arbitrage Bot Status:");
    info!("  • Transactions processed: {}", stats.transactions_processed);
    info!("  • Opportunities detected: {}", stats.opportunities_detected);
    info!("  • Total profit: {:.4} SOL", stats.total_profit_sol);

    // Show supported DEX pairs for arbitrage
    info!("💱 Arbitrage Trading Pairs:");
    info!("  • Supported DEXs: 26 (Raydium, Orca, Jupiter, Meteora, etc.)");
    info!("  • Trading pairs: 190+ cross-DEX opportunities");
    info!("  • Min profit threshold: 0.05 SOL");
    info!("  • Target execution: <3000ms");

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
    info!("🚀 Arbitrage Bot is now LIVE - Scanning for price differences...");
    info!("💡 Press Ctrl+C to stop");
    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // Start monitoring (this runs until shutdown)
    tokio::select! {
        result = arb_monitor.start_monitoring() => {
            match result {
                Ok(()) => info!("✅ Arbitrage monitoring completed successfully"),
                Err(e) => {
                    error!("❌ Arbitrage monitoring failed: {}", e);
                    return Err(e);
                }
            }
        }
        _ = shutdown_handle => {
            info!("🛑 Graceful shutdown initiated");
        }
    }

    // Final statistics before shutdown
    let final_stats = arb_monitor.get_stats();
    info!("📊 Final Arbitrage Bot Statistics:");
    info!("  • Runtime: {} seconds", final_stats.uptime_seconds);
    info!("  • Transactions processed: {}", final_stats.transactions_processed);
    info!("  • Opportunities detected: {}", final_stats.opportunities_detected);
    info!("  • Opportunities executed: {}", final_stats.opportunities_executed);
    info!("  • Total profit: {:.4} SOL", final_stats.total_profit_sol);
    info!("  • Average processing time: {:.2}ms", final_stats.average_processing_time_ms);

    let success_rate = if final_stats.opportunities_detected > 0 {
        (final_stats.opportunities_executed as f64 / final_stats.opportunities_detected as f64) * 100.0
    } else {
        0.0
    };
    info!("  • Success rate: {:.1}%", success_rate);

    // Generate final performance report
    if let Ok(report) = arb_monitor.generate_performance_report(24).await {
        info!("📋 24-Hour Arbitrage Performance:");
        info!("  • Total opportunities: {}", report.total_opportunities);
        info!("  • Total executions: {}", report.total_executions);
        info!("  • Total profit: {:.4} SOL", report.total_profit_sol);
        info!("  • Average execution time: {:.1}ms", report.average_execution_time_ms);
        info!("  • Success rate: {:.1}%", report.success_rate_percent);

        // Show profit breakdown (should be mostly arbitrage)
        for (engine, profit) in &report.profit_by_engine {
            if *profit > 0.0 {
                info!("  • {} profit: {:.4} SOL", engine, profit);
            }
        }
    }

    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    info!("👋 Arbitrage Bot shutdown complete");

    Ok(())
}