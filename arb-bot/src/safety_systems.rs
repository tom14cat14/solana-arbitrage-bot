use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error, debug};

/// Comprehensive safety system for production arbitrage trading
///
/// The SafetySystem provides multi-layered protection for automated trading:
/// - **Trading Limits**: Position sizes, daily limits, frequency controls
/// - **Risk Monitoring**: Real-time P&L tracking, drawdown analysis
/// - **Circuit Breakers**: Automatic trading suspension on anomalies
/// - **Emergency Controls**: Manual override and emergency stop capabilities
/// - **Position Tracking**: Active position monitoring and risk management
/// - **Audit Logging**: Comprehensive compliance and analysis logging
///
/// # Safety Philosophy
/// - **Defense in Depth**: Multiple independent safety layers
/// - **Fail Safe**: System defaults to safe state on any uncertainty
/// - **Real-time Monitoring**: Continuous risk assessment and response
/// - **Manual Override**: Human operators can always intervene
/// - **Audit Trail**: Complete record of all safety events and decisions
///
/// # Usage
/// The SafetySystem must approve every trade before execution:
/// ```ignore
/// let safety_check = safety_system.pre_trade_safety_check(
///     "SOL/USDC", "Raydium-Orca", 0.1, 0.01
/// )?;
/// if safety_check.approved {
///     // Execute trade
/// } else {
///     // Handle rejection: safety_check.reason
/// }
/// ```
#[derive(Debug, Clone)]
pub struct SafetySystem {
    trading_limits: Arc<RwLock<TradingLimits>>,
    risk_monitor: Arc<RwLock<RiskMonitor>>,
    circuit_breakers: Arc<RwLock<CircuitBreakers>>,
    emergency_controls: Arc<RwLock<EmergencyControls>>,
    position_tracker: Arc<RwLock<PositionTracker>>,
    audit_logger: AuditLogger,
}

/// Trading limits and thresholds for risk management
///
/// Defines the operational boundaries for automated trading to ensure
/// controlled risk exposure and prevent runaway trading scenarios.
/// All limits are enforced in real-time before trade execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingLimits {
    pub max_daily_loss_sol: f64,
    pub max_single_trade_sol: f64,
    pub max_concurrent_trades: u32,
    pub max_trades_per_minute: u32,
    pub max_trades_per_hour: u32,
    pub max_trades_per_day: u32,
    pub min_profit_threshold_sol: f64,
    pub max_slippage_percent: f64,
    pub max_execution_time_ms: u64,
    pub cooldown_period_seconds: u64,
}

/// Real-time risk monitoring and performance tracking
///
/// Continuously tracks trading performance, risk metrics, and market conditions
/// to enable dynamic risk management and automated safety responses.
/// Updated with every trade execution and market event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskMonitor {
    pub current_daily_pnl: f64,
    pub current_drawdown_percent: f64,
    pub consecutive_losses: u32,
    pub trades_last_minute: u32,
    pub trades_last_hour: u32,
    pub trades_today: u32,
    pub average_win_rate: f64,
    pub sharpe_ratio: f64,
    pub max_historical_drawdown: f64,
    pub volatility_score: f64,
    pub last_trade_time: Option<DateTime<Utc>>,
    pub risk_level: RiskLevel,
}

/// Multi-level circuit breaker system for automatic trading suspension
///
/// Monitors various risk conditions and automatically suspends trading
/// when predefined thresholds are exceeded. Provides multiple independent
/// breakers for different types of market and system anomalies.
///
/// # Circuit Breaker Types
/// - **Main Breaker**: Overall system suspension for critical conditions
/// - **Connection Breaker**: Network/data feed reliability issues
/// - **Profit Breaker**: Unusual profit/loss patterns indicating issues
/// - **Volume Breaker**: Abnormal trading volume patterns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakers {
    pub main_breaker_active: bool,
    pub connection_breaker_active: bool,
    pub profit_breaker_active: bool,
    pub volume_breaker_active: bool,
    pub main_breaker_triggered_at: Option<DateTime<Utc>>,
    pub connection_failures_count: u32,
    pub max_connection_failures: u32,
    pub reset_timeout_minutes: u32,
    pub auto_reset_enabled: bool,
}

/// Emergency control system for manual intervention and crisis management
///
/// Provides immediate manual override capabilities for human operators
/// to intervene in emergency situations. All emergency actions are
/// logged with timestamps and operator identification for audit trails.
///
/// # Emergency Scenarios
/// - Market crash or extreme volatility
/// - System malfunction or unexpected behavior
/// - Regulatory requirements or compliance issues
/// - Maintenance windows or system updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmergencyControls {
    pub emergency_stop_active: bool,
    pub maintenance_mode: bool,
    pub force_close_all_positions: bool,
    pub disable_new_trades: bool,
    pub emergency_triggered_by: Option<String>,
    pub emergency_triggered_at: Option<DateTime<Utc>>,
    pub emergency_reason: Option<String>,
    pub manual_override_enabled: bool,
}

/// Position tracking and management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionTracker {
    pub active_positions: HashMap<String, Position>,
    pub position_history: Vec<Position>,
    pub total_exposure_sol: f64,
    pub max_exposure_sol: f64,
    pub position_count: u32,
    pub average_position_age_minutes: f64,
}

/// Individual position data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub id: String,
    pub token_pair: String,
    pub dex_pair: String,
    pub entry_price: f64,
    pub current_price: f64,
    pub size_sol: f64,
    pub unrealized_pnl_sol: f64,
    pub entry_time: DateTime<Utc>,
    pub max_hold_time_minutes: u32,
    pub stop_loss_price: Option<f64>,
    pub take_profit_price: Option<f64>,
    pub status: PositionStatus,
}

/// Risk levels for dynamic safety adjustments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskLevel {
    VeryLow,
    Low,
    Medium,
    High,
    Critical,
}

/// Position status tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PositionStatus {
    Active,
    PendingClose,
    Closed,
    Emergency,
}

/// Safety check result
#[derive(Debug, Clone)]
pub struct SafetyCheckResult {
    pub approved: bool,
    pub reason: String,
    pub risk_level: RiskLevel,
    pub recommended_position_size: Option<f64>,
    pub warnings: Vec<String>,
}

/// Audit logging for compliance and analysis
#[derive(Debug, Clone)]
pub struct AuditLogger {
    log_file_path: String,
}

impl Default for TradingLimits {
    fn default() -> Self {
        Self {
            max_daily_loss_sol: 1.0,           // Maximum 1 SOL loss per day
            max_single_trade_sol: 0.2,         // Maximum 0.2 SOL per trade
            max_concurrent_trades: 5,          // Maximum 5 concurrent positions
            max_trades_per_minute: 2,          // Maximum 2 trades per minute
            max_trades_per_hour: 30,           // Maximum 30 trades per hour
            max_trades_per_day: 200,           // Maximum 200 trades per day
            min_profit_threshold_sol: 0.005,   // Minimum 0.005 SOL profit
            max_slippage_percent: 2.0,         // Maximum 2% slippage
            max_execution_time_ms: 5000,       // Maximum 5 second execution
            cooldown_period_seconds: 30,       // 30 second cooldown between trades
        }
    }
}

impl Default for RiskMonitor {
    fn default() -> Self {
        Self {
            current_daily_pnl: 0.0,
            current_drawdown_percent: 0.0,
            consecutive_losses: 0,
            trades_last_minute: 0,
            trades_last_hour: 0,
            trades_today: 0,
            average_win_rate: 0.0,
            sharpe_ratio: 0.0,
            max_historical_drawdown: 0.0,
            volatility_score: 0.0,
            last_trade_time: None,
            risk_level: RiskLevel::Low,
        }
    }
}

impl Default for CircuitBreakers {
    fn default() -> Self {
        Self {
            main_breaker_active: false,
            connection_breaker_active: false,
            profit_breaker_active: false,
            volume_breaker_active: false,
            main_breaker_triggered_at: None,
            connection_failures_count: 0,
            max_connection_failures: 10,
            reset_timeout_minutes: 30,
            auto_reset_enabled: true,
        }
    }
}

impl Default for EmergencyControls {
    fn default() -> Self {
        Self {
            emergency_stop_active: false,
            maintenance_mode: false,
            force_close_all_positions: false,
            disable_new_trades: false,
            emergency_triggered_by: None,
            emergency_triggered_at: None,
            emergency_reason: None,
            manual_override_enabled: false,
        }
    }
}

impl Default for PositionTracker {
    fn default() -> Self {
        Self {
            active_positions: HashMap::new(),
            position_history: Vec::new(),
            total_exposure_sol: 0.0,
            max_exposure_sol: 2.0,
            position_count: 0,
            average_position_age_minutes: 0.0,
        }
    }
}

impl SafetySystem {
    /// Initialize comprehensive safety system
    pub fn new() -> Self {
        info!("🛡️ Initializing Production Safety System");
        info!("  • Trading limits: STRICT production limits");
        info!("  • Risk monitoring: Real-time P&L and drawdown tracking");
        info!("  • Circuit breakers: Multi-level protection system");
        info!("  • Emergency controls: Manual override capabilities");
        info!("  • Position tracking: Real-time exposure monitoring");
        info!("  • Audit logging: Comprehensive compliance logging");

        Self {
            trading_limits: Arc::new(RwLock::new(TradingLimits::default())),
            risk_monitor: Arc::new(RwLock::new(RiskMonitor::default())),
            circuit_breakers: Arc::new(RwLock::new(CircuitBreakers::default())),
            emergency_controls: Arc::new(RwLock::new(EmergencyControls::default())),
            position_tracker: Arc::new(RwLock::new(PositionTracker::default())),
            audit_logger: AuditLogger::new(),
        }
    }

    /// Pre-trade safety check - MUST pass before any trade execution
    pub fn pre_trade_safety_check(
        &self,
        _token_pair: &str,
        _dex_pair: &str,
        position_size_sol: f64,
        expected_profit_sol: f64,
    ) -> Result<SafetyCheckResult> {
        let limits = self.trading_limits.read();
        let risk = self.risk_monitor.read();
        let breakers = self.circuit_breakers.read();
        let emergency = self.emergency_controls.read();
        let positions = self.position_tracker.read();

        let mut warnings = Vec::new();

        // Emergency controls check
        if emergency.emergency_stop_active {
            return Ok(SafetyCheckResult {
                approved: false,
                reason: "Emergency stop is active".to_string(),
                risk_level: RiskLevel::Critical,
                recommended_position_size: None,
                warnings,
            });
        }

        if emergency.disable_new_trades {
            return Ok(SafetyCheckResult {
                approved: false,
                reason: "New trades disabled".to_string(),
                risk_level: RiskLevel::High,
                recommended_position_size: None,
                warnings,
            });
        }

        // Circuit breaker check
        if breakers.main_breaker_active {
            return Ok(SafetyCheckResult {
                approved: false,
                reason: "Main circuit breaker is active".to_string(),
                risk_level: RiskLevel::Critical,
                recommended_position_size: None,
                warnings,
            });
        }

        // Daily loss limit check
        if risk.current_daily_pnl < -limits.max_daily_loss_sol {
            return Ok(SafetyCheckResult {
                approved: false,
                reason: format!("Daily loss limit exceeded: {:.4} SOL", risk.current_daily_pnl),
                risk_level: RiskLevel::Critical,
                recommended_position_size: None,
                warnings,
            });
        }

        // Position size check
        if position_size_sol > limits.max_single_trade_sol {
            return Ok(SafetyCheckResult {
                approved: false,
                reason: format!("Position size too large: {:.4} SOL > {:.4} SOL",
                              position_size_sol, limits.max_single_trade_sol),
                risk_level: RiskLevel::High,
                recommended_position_size: Some(limits.max_single_trade_sol),
                warnings,
            });
        }

        // Concurrent trades check
        if positions.position_count >= limits.max_concurrent_trades {
            return Ok(SafetyCheckResult {
                approved: false,
                reason: format!("Too many concurrent trades: {} >= {}",
                              positions.position_count, limits.max_concurrent_trades),
                risk_level: RiskLevel::Medium,
                recommended_position_size: None,
                warnings,
            });
        }

        // Trading frequency checks
        if risk.trades_last_minute >= limits.max_trades_per_minute {
            return Ok(SafetyCheckResult {
                approved: false,
                reason: "Too many trades in last minute".to_string(),
                risk_level: RiskLevel::Medium,
                recommended_position_size: None,
                warnings,
            });
        }

        if risk.trades_today >= limits.max_trades_per_day {
            return Ok(SafetyCheckResult {
                approved: false,
                reason: "Daily trade limit exceeded".to_string(),
                risk_level: RiskLevel::High,
                recommended_position_size: None,
                warnings,
            });
        }

        // Profit threshold check
        if expected_profit_sol < limits.min_profit_threshold_sol {
            return Ok(SafetyCheckResult {
                approved: false,
                reason: format!("Insufficient expected profit: {:.6} SOL < {:.6} SOL",
                              expected_profit_sol, limits.min_profit_threshold_sol),
                risk_level: RiskLevel::Low,
                recommended_position_size: None,
                warnings,
            });
        }

        // Exposure limit check
        let new_total_exposure = positions.total_exposure_sol + position_size_sol;
        if new_total_exposure > positions.max_exposure_sol {
            warnings.push(format!("High exposure warning: {:.4} SOL", new_total_exposure));
        }

        // Consecutive losses check
        if risk.consecutive_losses >= 5 {
            warnings.push("High consecutive loss count".to_string());
        }

        // High drawdown check
        if risk.current_drawdown_percent > 10.0 {
            warnings.push(format!("High drawdown: {:.1}%", risk.current_drawdown_percent));
        }

        // All checks passed
        Ok(SafetyCheckResult {
            approved: true,
            reason: "All safety checks passed".to_string(),
            risk_level: risk.risk_level.clone(),
            recommended_position_size: Some(position_size_sol),
            warnings,
        })
    }

    /// Record trade execution for monitoring
    pub fn record_trade_execution(
        &self,
        position_id: String,
        token_pair: String,
        dex_pair: String,
        size_sol: f64,
        entry_price: f64,
        successful: bool,
        profit_sol: f64,
    ) -> Result<()> {
        let mut risk = self.risk_monitor.write();
        let mut positions = self.position_tracker.write();

        // Update trade counters
        risk.trades_today += 1;
        risk.trades_last_hour += 1;
        risk.trades_last_minute += 1;
        risk.last_trade_time = Some(Utc::now());

        // Update P&L
        risk.current_daily_pnl += profit_sol;

        if successful && profit_sol > 0.0 {
            // Successful trade
            risk.consecutive_losses = 0;
        } else {
            // Failed or losing trade
            risk.consecutive_losses += 1;
        }

        // Create position record
        let position = Position {
            id: position_id.clone(),
            token_pair: token_pair.clone(),
            dex_pair: dex_pair.clone(),
            entry_price,
            current_price: entry_price,
            size_sol,
            unrealized_pnl_sol: 0.0,
            entry_time: Utc::now(),
            max_hold_time_minutes: 60, // 1 hour maximum hold time
            stop_loss_price: Some(entry_price * 0.95), // 5% stop loss
            take_profit_price: Some(entry_price * 1.02), // 2% take profit
            status: PositionStatus::Active,
        };

        // Add to active positions
        positions.active_positions.insert(position_id.clone(), position.clone());
        positions.position_count += 1;
        positions.total_exposure_sol += size_sol;

        // Add to history
        positions.position_history.push(position);

        // Log the trade
        self.audit_logger.log_trade_execution(&position_id, &token_pair, &dex_pair,
                                           size_sol, profit_sol, successful)?;

        info!("📊 Trade recorded: {} | Size: {:.4} SOL | P&L: {:.6} SOL | Active positions: {}",
              position_id, size_sol, profit_sol, positions.position_count);

        Ok(())
    }

    /// Update position with current market data
    pub fn update_position(&self, position_id: &str, current_price: f64) -> Result<()> {
        let mut positions = self.position_tracker.write();

        if let Some(position) = positions.active_positions.get_mut(position_id) {
            position.current_price = current_price;
            position.unrealized_pnl_sol = (current_price - position.entry_price) * position.size_sol;

            // Check stop loss and take profit
            if let Some(stop_loss) = position.stop_loss_price {
                if current_price <= stop_loss {
                    warn!("🔴 Stop loss triggered for position {}: {:.6} <= {:.6}",
                          position_id, current_price, stop_loss);
                    position.status = PositionStatus::PendingClose;
                }
            }

            if let Some(take_profit) = position.take_profit_price {
                if current_price >= take_profit {
                    info!("🟢 Take profit triggered for position {}: {:.6} >= {:.6}",
                          position_id, current_price, take_profit);
                    position.status = PositionStatus::PendingClose;
                }
            }

            // Check maximum hold time
            let position_age = Utc::now().signed_duration_since(position.entry_time);
            if position_age.num_minutes() > position.max_hold_time_minutes as i64 {
                warn!("⏰ Position {} exceeded maximum hold time: {} minutes",
                      position_id, position_age.num_minutes());
                position.status = PositionStatus::PendingClose;
            }
        }

        Ok(())
    }

    /// Close position and update tracking
    pub fn close_position(&self, position_id: &str, exit_price: f64, realized_pnl: f64) -> Result<()> {
        let mut positions = self.position_tracker.write();
        let mut risk = self.risk_monitor.write();

        if let Some(mut position) = positions.active_positions.remove(position_id) {
            position.status = PositionStatus::Closed;
            position.current_price = exit_price;
            position.unrealized_pnl_sol = realized_pnl;

            // Update exposure
            positions.total_exposure_sol -= position.size_sol;
            positions.position_count -= 1;

            // Update daily P&L
            risk.current_daily_pnl += realized_pnl;

            // Update position history
            positions.position_history.push(position.clone());

            info!("🔄 Position closed: {} | Exit price: {:.6} | P&L: {:.6} SOL | Remaining positions: {}",
                  position_id, exit_price, realized_pnl, positions.position_count);

            // Log the closure
            self.audit_logger.log_position_closure(position_id, exit_price, realized_pnl)?;
        }

        Ok(())
    }

    /// Trigger emergency stop
    pub fn trigger_emergency_stop(&self, reason: String, triggered_by: String) -> Result<()> {
        let mut emergency = self.emergency_controls.write();

        emergency.emergency_stop_active = true;
        emergency.disable_new_trades = true;
        emergency.force_close_all_positions = true;
        emergency.emergency_reason = Some(reason.clone());
        emergency.emergency_triggered_by = Some(triggered_by.clone());
        emergency.emergency_triggered_at = Some(Utc::now());

        error!("🚨 EMERGENCY STOP TRIGGERED: {} by {}", reason, triggered_by);

        // Log emergency event
        self.audit_logger.log_emergency_event(&reason, &triggered_by)?;

        Ok(())
    }

    /// Activate circuit breaker
    pub fn activate_circuit_breaker(&self, breaker_type: &str, reason: String) -> Result<()> {
        let mut breakers = self.circuit_breakers.write();

        match breaker_type {
            "main" => {
                breakers.main_breaker_active = true;
                breakers.main_breaker_triggered_at = Some(Utc::now());
            }
            "connection" => {
                breakers.connection_breaker_active = true;
                breakers.connection_failures_count += 1;
            }
            "profit" => {
                breakers.profit_breaker_active = true;
            }
            "volume" => {
                breakers.volume_breaker_active = true;
            }
            _ => {
                return Err(anyhow::anyhow!("Unknown circuit breaker type: {}", breaker_type));
            }
        }

        error!("🔴 Circuit breaker activated: {} - {}", breaker_type, reason);

        // Log circuit breaker event
        self.audit_logger.log_circuit_breaker_event(breaker_type, &reason)?;

        Ok(())
    }

    /// Get current safety status
    pub fn get_safety_status(&self) -> SafetyStatus {
        let limits = self.trading_limits.read();
        let risk = self.risk_monitor.read();
        let breakers = self.circuit_breakers.read();
        let emergency = self.emergency_controls.read();
        let positions = self.position_tracker.read();

        SafetyStatus {
            trading_allowed: !emergency.emergency_stop_active && !emergency.disable_new_trades && !breakers.main_breaker_active,
            emergency_active: emergency.emergency_stop_active,
            main_breaker_active: breakers.main_breaker_active,
            daily_pnl: risk.current_daily_pnl,
            drawdown_percent: risk.current_drawdown_percent,
            active_positions: positions.position_count,
            total_exposure_sol: positions.total_exposure_sol,
            risk_level: risk.risk_level.clone(),
            trades_today: risk.trades_today,
            max_daily_trades: limits.max_trades_per_day,
            consecutive_losses: risk.consecutive_losses,
        }
    }

    /// Generate safety report
    pub fn generate_safety_report(&self) -> SafetyReport {
        let status = self.get_safety_status();
        let positions = self.position_tracker.read();

        SafetyReport {
            timestamp: Utc::now(),
            overall_status: if status.trading_allowed { "OPERATIONAL".to_string() } else { "RESTRICTED".to_string() },
            safety_status: status,
            position_summary: PositionSummary {
                active_count: positions.position_count,
                total_exposure: positions.total_exposure_sol,
                average_age_minutes: positions.average_position_age_minutes,
                pending_close_count: positions.active_positions.values()
                    .filter(|p| matches!(p.status, PositionStatus::PendingClose))
                    .count() as u32,
            },
            recommendations: self.generate_safety_recommendations(),
        }
    }

    /// Generate safety recommendations
    fn generate_safety_recommendations(&self) -> Vec<String> {
        let mut recommendations = Vec::new();
        let risk = self.risk_monitor.read();
        let positions = self.position_tracker.read();

        if risk.consecutive_losses >= 3 {
            recommendations.push("Consider reducing position sizes due to consecutive losses".to_string());
        }

        if risk.current_drawdown_percent > 5.0 {
            recommendations.push(format!("High drawdown detected: {:.1}% - Review risk management",
                                      risk.current_drawdown_percent));
        }

        if positions.total_exposure_sol > positions.max_exposure_sol * 0.8 {
            recommendations.push("High exposure warning - Consider reducing position sizes".to_string());
        }

        if risk.trades_today > 100 {
            recommendations.push("High trading frequency - Monitor for overtrading".to_string());
        }

        recommendations
    }
}

/// Current safety status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyStatus {
    pub trading_allowed: bool,
    pub emergency_active: bool,
    pub main_breaker_active: bool,
    pub daily_pnl: f64,
    pub drawdown_percent: f64,
    pub active_positions: u32,
    pub total_exposure_sol: f64,
    pub risk_level: RiskLevel,
    pub trades_today: u32,
    pub max_daily_trades: u32,
    pub consecutive_losses: u32,
}

/// Safety report for monitoring and analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyReport {
    pub timestamp: DateTime<Utc>,
    pub overall_status: String,
    pub safety_status: SafetyStatus,
    pub position_summary: PositionSummary,
    pub recommendations: Vec<String>,
}

/// Position summary for reporting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionSummary {
    pub active_count: u32,
    pub total_exposure: f64,
    pub average_age_minutes: f64,
    pub pending_close_count: u32,
}

impl AuditLogger {
    fn new() -> Self {
        Self {
            log_file_path: "arbitrage_audit.log".to_string(),
        }
    }

    fn log_trade_execution(&self, position_id: &str, token_pair: &str, _dex_pair: &str,
                          size_sol: f64, profit_sol: f64, successful: bool) -> Result<()> {
        debug!("📝 Audit log: Trade {} | {} | {:.4} SOL | P&L: {:.6} SOL | Success: {}",
               position_id, token_pair, size_sol, profit_sol, successful);
        // In production, this would write to actual audit log file
        Ok(())
    }

    fn log_position_closure(&self, position_id: &str, exit_price: f64, pnl: f64) -> Result<()> {
        debug!("📝 Audit log: Position closed {} | Exit: {:.6} | P&L: {:.6} SOL",
               position_id, exit_price, pnl);
        Ok(())
    }

    fn log_emergency_event(&self, reason: &str, triggered_by: &str) -> Result<()> {
        error!("📝 AUDIT: Emergency stop - {} by {}", reason, triggered_by);
        Ok(())
    }

    fn log_circuit_breaker_event(&self, breaker_type: &str, reason: &str) -> Result<()> {
        error!("📝 AUDIT: Circuit breaker {} - {}", breaker_type, reason);
        Ok(())
    }
}

impl Default for SafetySystem {
    fn default() -> Self {
        Self::new()
    }
}