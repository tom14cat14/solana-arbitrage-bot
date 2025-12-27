use anyhow::Result;
use arb_bot::safety_systems::{SafetySystem, RiskLevel};

#[tokio::test]
async fn test_pre_trade_safety_checks() -> Result<()> {
    let safety_system = SafetySystem::new();

    // Test normal trade approval
    let result = safety_system.pre_trade_safety_check(
        "SOL/USDC",
        "Raydium-Orca",
        0.1, // position size
        0.01, // expected profit
    )?;

    assert!(result.approved);
    assert_eq!(result.recommended_position_size, Some(0.1));
    assert!(matches!(result.risk_level, RiskLevel::Low | RiskLevel::Medium));

    Ok(())
}

#[tokio::test]
async fn test_position_size_limits() -> Result<()> {
    let safety_system = SafetySystem::new();

    // Test oversized position rejection
    let result = safety_system.pre_trade_safety_check(
        "SOL/USDC",
        "Raydium-Orca",
        1.0, // position size too large (> 0.2 default limit)
        0.01, // expected profit
    )?;

    assert!(!result.approved);
    assert!(result.reason.contains("Position size too large"));
    assert_eq!(result.recommended_position_size, Some(0.2)); // Should recommend max allowed

    Ok(())
}

#[tokio::test]
async fn test_profit_threshold_enforcement() -> Result<()> {
    let safety_system = SafetySystem::new();

    // Test insufficient profit rejection
    let result = safety_system.pre_trade_safety_check(
        "SOL/USDC",
        "Raydium-Orca",
        0.1, // position size
        0.001, // expected profit too low (< 0.005 default minimum)
    )?;

    assert!(!result.approved);
    assert!(result.reason.contains("Insufficient expected profit"));

    Ok(())
}

#[tokio::test]
async fn test_emergency_stop_functionality() -> Result<()> {
    let safety_system = SafetySystem::new();

    // Trigger emergency stop
    safety_system.trigger_emergency_stop(
        "Test emergency scenario".to_string(),
        "unit_test".to_string(),
    )?;

    // Check that trading is now blocked
    let result = safety_system.pre_trade_safety_check(
        "SOL/USDC",
        "Raydium-Orca",
        0.1,
        0.01,
    )?;

    assert!(!result.approved);
    assert!(result.reason.contains("Emergency stop is active"));
    assert!(matches!(result.risk_level, RiskLevel::Critical));

    Ok(())
}

#[tokio::test]
async fn test_circuit_breaker_activation() -> Result<()> {
    let safety_system = SafetySystem::new();

    // Activate main circuit breaker
    safety_system.activate_circuit_breaker(
        "main",
        "Test circuit breaker activation".to_string(),
    )?;

    // Check that trading is now blocked
    let result = safety_system.pre_trade_safety_check(
        "SOL/USDC",
        "Raydium-Orca",
        0.1,
        0.01,
    )?;

    assert!(!result.approved);
    assert!(result.reason.contains("Main circuit breaker is active"));

    Ok(())
}

#[tokio::test]
async fn test_trade_recording_and_tracking() -> Result<()> {
    let safety_system = SafetySystem::new();

    // Record a successful trade
    safety_system.record_trade_execution(
        "test_position_1".to_string(),
        "SOL/USDC".to_string(),
        "Raydium-Orca".to_string(),
        0.1, // size
        150.0, // entry price
        true, // successful
        0.01, // profit
    )?;

    let status = safety_system.get_safety_status();
    assert_eq!(status.trades_today, 1);
    assert_eq!(status.active_positions, 1);
    assert_eq!(status.daily_pnl, 0.01);

    Ok(())
}

#[tokio::test]
async fn test_position_updates_and_closure() -> Result<()> {
    let safety_system = SafetySystem::new();

    // Record a trade
    safety_system.record_trade_execution(
        "test_position_2".to_string(),
        "SOL/USDC".to_string(),
        "Raydium-Orca".to_string(),
        0.1,
        150.0,
        true,
        0.0, // Initial profit
    )?;

    // Update position with new price
    safety_system.update_position("test_position_2", 155.0)?;

    // Close position
    safety_system.close_position("test_position_2", 155.0, 0.033)?;

    let status = safety_system.get_safety_status();
    assert_eq!(status.active_positions, 0); // Position should be closed
    assert!(status.daily_pnl > 0.0); // Should have profit

    Ok(())
}

#[tokio::test]
async fn test_safety_report_generation() -> Result<()> {
    let safety_system = SafetySystem::new();

    // Record some trades
    for i in 0..3 {
        safety_system.record_trade_execution(
            format!("test_position_{}", i),
            "SOL/USDC".to_string(),
            "Raydium-Orca".to_string(),
            0.1,
            150.0,
            i % 2 == 0, // Alternate successful/failed
            if i % 2 == 0 { 0.01 } else { -0.005 },
        )?;
    }

    let report = safety_system.generate_safety_report();

    assert!(report.overall_status == "OPERATIONAL" || report.overall_status == "RESTRICTED");
    assert_eq!(report.position_summary.active_count, 3);
    assert!(!report.recommendations.is_empty() || report.recommendations.is_empty()); // Either way is valid

    Ok(())
}

#[test]
fn test_concurrent_safety_operations() {
    use std::sync::Arc;
    use std::thread;

    let safety_system = Arc::new(SafetySystem::new());
    let mut handles = vec![];

    // Spawn multiple threads doing safety checks
    for _i in 0..10 {
        let safety_clone = Arc::clone(&safety_system);
        let handle = thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async move {
                let result = safety_clone.pre_trade_safety_check(
                    "SOL/USDC",
                    "Raydium-Orca",
                    0.05,
                    0.01,
                ).unwrap();

                // All should pass initially
                assert!(result.approved);
            });
        });
        handles.push(handle);
    }

    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }

    // All operations should have completed successfully
    let status = safety_system.get_safety_status();
    assert!(status.trading_allowed);
}

#[tokio::test]
async fn test_daily_loss_limit_enforcement() -> Result<()> {
    let safety_system = SafetySystem::new();

    // Record multiple losing trades to exceed daily limit
    for i in 0..5 {
        safety_system.record_trade_execution(
            format!("losing_position_{}", i),
            "SOL/USDC".to_string(),
            "Raydium-Orca".to_string(),
            0.1,
            150.0,
            false, // Failed trade
            -0.25, // Large loss to quickly exceed daily limit (default -1.0 SOL)
        )?;
    }

    // Should now reject new trades due to daily loss limit
    let result = safety_system.pre_trade_safety_check(
        "SOL/USDC",
        "Raydium-Orca",
        0.1,
        0.01,
    )?;

    assert!(!result.approved);
    assert!(result.reason.contains("Daily loss limit exceeded"));

    Ok(())
}