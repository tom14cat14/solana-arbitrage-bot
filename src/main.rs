//! Clean Arbitrage Bot - Production MEV Trading System
//! CYCLE-7: Grok-approved production system (9/10 → 10/10 in progress)

use anyhow::Result;
use tokio::signal;
use tokio::sync::broadcast;
use tracing::{error, info};

mod arbitrage_engine;
mod config;
mod dex_registry;
mod jito_bundle_client;
mod jito_grpc_client; // NEW (2025-10-12): gRPC for 75ms faster submission!
mod jito_submitter;
mod jito_tip_monitor;
mod jupiter_prices;
mod jupiter_triangle;
mod shredstream_client;
mod simple_triangle_detector;
mod triangle_arbitrage; // NEW: Dynamic JITO tip adjustment (every 30 min)
                        // DEX swap modules (flattened from dex_swap/ directory)
mod humidifi;
mod meteora;
mod orca;
mod pool_registry;
mod pumpswap;
mod raydium;
mod rpc_client;
mod swap_executor;
mod types;

mod cached_blockhash;
mod cost_calculator; // Cost calculation and profitability filtering
mod meteora_swap; // CYCLE-7: Meteora DAMM V2 swap instructions (90% of opportunities)
mod pool_population;
mod position_tracker; // HIGH-4 FIX: Position tracking module
mod slippage; // CYCLE-7: Dynamic slippage protection // NEW (2025-10-11): Pre-fetched blockhash (saves 50-70ms per tx)

// Public re-exports for convenience (previously in dex_swap/mod.rs)
use pool_registry::PoolRegistry;
use rpc_client::SolanaRpcClient;
use swap_executor::SwapExecutor;
use types::{extract_pool_id, DexType, PoolInfo, SwapParams};

use arbitrage_engine::ArbitrageEngine;
use config::Config;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("info,clean_arb_bot=debug")
        .init();

    info!("💰 Starting Clean Arbitrage Bot");
    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // Load configuration
    let config = Config::from_env()?;

    info!("✅ Configuration loaded:");
    info!("  • ShredStream service: {}", config.shredstream_url);
    info!("  • Capital: {:.2} SOL", config.capital_sol);
    info!(
        "  • Max position: {:.2} SOL ({:.0}% of tradable capital)",
        config.max_position_size_sol,
        (config.max_position_size_sol / config.capital_sol) * 100.0
    );
    info!("  • Profit requirement: Dynamic (costs + 0.2% margin calculated per opportunity)");
    info!("  • Min spread: DYNAMIC (calculated per opportunity: [total_costs + margin] / position_size)");
    info!(
        "  • Trading mode: {}",
        if config.paper_trading {
            "PAPER"
        } else {
            "LIVE"
        }
    );

    // Create shutdown channel (Grok recommendation: explicit shutdown signaling)
    let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

    // Spawn JITO tip floor monitor (updates every 30 minutes)
    info!("📊 Starting JITO tip floor monitor...");
    let jito_tip_floor = jito_tip_monitor::spawn_monitor();
    info!("✅ JITO tip monitor started (dynamic competitive tipping)");

    // Create arbitrage engine with shutdown receiver and tip floor
    info!("🚀 Initializing arbitrage engine...");
    let mut engine = ArbitrageEngine::new(config.clone(), shutdown_rx, jito_tip_floor).await?;
    info!("✅ Arbitrage engine ready");

    // Populate pool registry if real trading is enabled
    if !config.paper_trading && config.enable_real_trading {
        if let Some(ref pool_registry) = engine.get_pool_registry() {
            info!("📋 Populating pool registry for real trading...");
            pool_population::populate_known_pools(pool_registry.clone())?;
        }
    }

    // Set up graceful shutdown handler (Grok recommendation: explicit error handling)
    let shutdown_handle = tokio::spawn(async move {
        match signal::ctrl_c().await {
            Ok(()) => {
                info!("🛑 Shutdown signal received (Ctrl+C)");
                let _ = shutdown_tx.send(()); // Signal engine to stop
                Ok(())
            }
            Err(err) => {
                error!("❌ Failed to listen for shutdown signal: {}", err);
                Err(err)
            }
        }
    });

    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    info!("🚀 Arbitrage Bot is LIVE - Scanning for opportunities...");
    info!("💡 Press Ctrl+C to stop gracefully");
    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // Run arbitrage engine (Grok recommendation: cooperative cancellation via shutdown channel)
    let engine_result = tokio::select! {
        result = engine.run() => {
            match result {
                Ok(()) => {
                    info!("✅ Arbitrage engine stopped normally");
                    Ok(())
                }
                Err(e) => {
                    error!("❌ Arbitrage engine failed: {}", e);
                    Err(e)
                }
            }
        }
        shutdown_result = shutdown_handle => {
            match shutdown_result {
                Ok(Ok(())) => {
                    info!("✅ Shutdown handler completed successfully");
                    Ok(())
                }
                Ok(Err(e)) => {
                    error!("❌ Shutdown handler failed: {}", e);
                    Err(anyhow::anyhow!("Shutdown handler error: {}", e))
                }
                Err(e) => {
                    error!("❌ Shutdown task panicked: {}", e);
                    Err(anyhow::anyhow!("Shutdown task panic: {}", e))
                }
            }
        }
    };

    // Allow engine to finish cleanup before accessing stats
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Final statistics (Grok recommendation: ensure thread-safe access post-cancellation)
    let stats = engine.get_stats();
    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    info!("📊 Final Statistics:");
    info!(
        "  • Runtime: {:.1} minutes",
        stats.runtime_seconds as f64 / 60.0
    );
    info!(
        "  • Opportunities detected: {}",
        stats.opportunities_detected
    );
    info!(
        "  • Opportunities executed: {}",
        stats.opportunities_executed
    );
    info!("  • Success rate: {:.1}%", stats.success_rate());
    info!("  • Total profit: {:.6} SOL", stats.total_profit_sol);
    info!("  • Failed executions: {}", stats.failed_executions);
    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    info!("👋 Arbitrage Bot shutdown complete");

    engine_result
}
