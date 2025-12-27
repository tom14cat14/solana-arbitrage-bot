use std::sync::Arc;
use parking_lot::RwLock;
use std::time::Instant;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

/// Performance metrics collector for arbitrage bot
#[derive(Debug, Clone)]
pub struct MetricsCollector {
    metrics: Arc<RwLock<PerformanceMetrics>>,
    start_time: Instant,
}

/// Comprehensive performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub shredstream_metrics: ShredStreamMetrics,
    pub arbitrage_metrics: ArbitrageMetrics,
    pub system_metrics: SystemMetrics,
    pub network_metrics: NetworkMetrics,
    pub trading_metrics: TradingMetrics,
}

/// ShredStream connection and data processing metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShredStreamMetrics {
    pub connections_established: u64,
    pub connection_failures: u64,
    pub reconnection_attempts: u64,
    pub successful_reconnections: u64,
    pub circuit_breaker_opens: u64,
    pub data_bytes_received: u64,
    pub data_bytes_processed: u64,
    pub protobuf_messages_parsed: u64,
    pub protobuf_parse_failures: u64,
    pub average_latency_ms: f64,
    pub peak_latency_ms: f64,
    pub last_data_received: Option<DateTime<Utc>>,
    pub uptime_seconds: u64,
}

/// Arbitrage detection and execution metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageMetrics {
    pub opportunities_detected: u64,
    pub opportunities_executed: u64,
    pub opportunities_failed: u64,
    pub profitable_trades: u64,
    pub losing_trades: u64,
    pub total_profit_sol: f64,
    pub total_fees_paid_sol: f64,
    pub average_execution_time_ms: f64,
    pub peak_execution_time_ms: f64,
    pub cross_dex_opportunities: u64,
    pub triangular_opportunities: u64,
    pub sandwich_opportunities: u64,
    pub mev_protection_activations: u64,
}

/// System resource utilization metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub cpu_usage_percent: f64,
    pub memory_usage_mb: f64,
    pub memory_usage_percent: f64,
    pub network_bytes_sent: u64,
    pub network_bytes_received: u64,
    pub disk_io_read_mb: f64,
    pub disk_io_write_mb: f64,
    pub active_connections: u32,
    pub thread_count: u32,
    pub file_descriptors: u32,
}

/// Network performance and health metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkMetrics {
    pub rpc_requests_total: u64,
    pub rpc_requests_successful: u64,
    pub rpc_requests_failed: u64,
    pub average_rpc_latency_ms: f64,
    pub peak_rpc_latency_ms: f64,
    pub jupiter_api_calls: u64,
    pub jupiter_api_failures: u64,
    pub jito_bundle_submissions: u64,
    pub jito_bundle_successes: u64,
    pub websocket_reconnections: u64,
    pub udp_packets_sent: u64,
    pub udp_packets_received: u64,
    pub udp_packet_loss_percent: f64,
}

/// Trading performance and risk metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingMetrics {
    pub positions_opened: u64,
    pub positions_closed: u64,
    pub positions_currently_open: u32,
    pub average_position_size_sol: f64,
    pub largest_position_size_sol: f64,
    pub average_hold_time_minutes: f64,
    pub longest_hold_time_minutes: f64,
    pub win_rate_percent: f64,
    pub profit_factor: f64, // Gross profit / Gross loss
    pub sharpe_ratio: f64,
    pub maximum_drawdown_percent: f64,
    pub current_drawdown_percent: f64,
    pub risk_score: f64, // 0-100 scale
}

/// Time-series data point for historical tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricDataPoint {
    pub timestamp: DateTime<Utc>,
    pub value: f64,
    pub label: String,
}

/// Historical metrics storage
#[derive(Debug, Clone)]
pub struct HistoricalMetrics {
    pub latency_history: Vec<MetricDataPoint>,
    pub profit_history: Vec<MetricDataPoint>,
    pub opportunity_history: Vec<MetricDataPoint>,
    pub error_rate_history: Vec<MetricDataPoint>,
    pub max_history_size: usize,
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self {
            shredstream_metrics: ShredStreamMetrics::default(),
            arbitrage_metrics: ArbitrageMetrics::default(),
            system_metrics: SystemMetrics::default(),
            network_metrics: NetworkMetrics::default(),
            trading_metrics: TradingMetrics::default(),
        }
    }
}

impl Default for ShredStreamMetrics {
    fn default() -> Self {
        Self {
            connections_established: 0,
            connection_failures: 0,
            reconnection_attempts: 0,
            successful_reconnections: 0,
            circuit_breaker_opens: 0,
            data_bytes_received: 0,
            data_bytes_processed: 0,
            protobuf_messages_parsed: 0,
            protobuf_parse_failures: 0,
            average_latency_ms: 0.0,
            peak_latency_ms: 0.0,
            last_data_received: None,
            uptime_seconds: 0,
        }
    }
}

impl Default for ArbitrageMetrics {
    fn default() -> Self {
        Self {
            opportunities_detected: 0,
            opportunities_executed: 0,
            opportunities_failed: 0,
            profitable_trades: 0,
            losing_trades: 0,
            total_profit_sol: 0.0,
            total_fees_paid_sol: 0.0,
            average_execution_time_ms: 0.0,
            peak_execution_time_ms: 0.0,
            cross_dex_opportunities: 0,
            triangular_opportunities: 0,
            sandwich_opportunities: 0,
            mev_protection_activations: 0,
        }
    }
}

impl Default for SystemMetrics {
    fn default() -> Self {
        Self {
            cpu_usage_percent: 0.0,
            memory_usage_mb: 0.0,
            memory_usage_percent: 0.0,
            network_bytes_sent: 0,
            network_bytes_received: 0,
            disk_io_read_mb: 0.0,
            disk_io_write_mb: 0.0,
            active_connections: 0,
            thread_count: 0,
            file_descriptors: 0,
        }
    }
}

impl Default for NetworkMetrics {
    fn default() -> Self {
        Self {
            rpc_requests_total: 0,
            rpc_requests_successful: 0,
            rpc_requests_failed: 0,
            average_rpc_latency_ms: 0.0,
            peak_rpc_latency_ms: 0.0,
            jupiter_api_calls: 0,
            jupiter_api_failures: 0,
            jito_bundle_submissions: 0,
            jito_bundle_successes: 0,
            websocket_reconnections: 0,
            udp_packets_sent: 0,
            udp_packets_received: 0,
            udp_packet_loss_percent: 0.0,
        }
    }
}

impl Default for TradingMetrics {
    fn default() -> Self {
        Self {
            positions_opened: 0,
            positions_closed: 0,
            positions_currently_open: 0,
            average_position_size_sol: 0.0,
            largest_position_size_sol: 0.0,
            average_hold_time_minutes: 0.0,
            longest_hold_time_minutes: 0.0,
            win_rate_percent: 0.0,
            profit_factor: 0.0,
            sharpe_ratio: 0.0,
            maximum_drawdown_percent: 0.0,
            current_drawdown_percent: 0.0,
            risk_score: 0.0,
        }
    }
}

impl MetricsCollector {
    /// Create new metrics collector
    pub fn new() -> Self {
        info!("📊 Initializing Enhanced Performance Metrics System");
        info!("  • ShredStream monitoring enabled");
        info!("  • Arbitrage performance tracking enabled");
        info!("  • System resource monitoring enabled");
        info!("  • Network health monitoring enabled");
        info!("  • Trading risk metrics enabled");

        Self {
            metrics: Arc::new(RwLock::new(PerformanceMetrics::default())),
            start_time: Instant::now(),
        }
    }

    /// Record ShredStream connection event
    pub fn record_shredstream_connection(&self, successful: bool) {
        let mut metrics = self.metrics.write();
        if successful {
            metrics.shredstream_metrics.connections_established += 1;
        } else {
            metrics.shredstream_metrics.connection_failures += 1;
        }
    }

    /// Record ShredStream reconnection attempt
    pub fn record_shredstream_reconnection(&self, successful: bool) {
        let mut metrics = self.metrics.write();
        metrics.shredstream_metrics.reconnection_attempts += 1;
        if successful {
            metrics.shredstream_metrics.successful_reconnections += 1;
        }
    }

    /// Record circuit breaker activation
    pub fn record_circuit_breaker_open(&self) {
        let mut metrics = self.metrics.write();
        metrics.shredstream_metrics.circuit_breaker_opens += 1;
        warn!("🔴 Circuit breaker opened - Total opens: {}",
              metrics.shredstream_metrics.circuit_breaker_opens);
    }

    /// Record data processing metrics
    pub fn record_data_processing(&self, bytes_received: usize, bytes_processed: usize, latency_ms: f64) {
        let mut metrics = self.metrics.write();
        metrics.shredstream_metrics.data_bytes_received += bytes_received as u64;
        metrics.shredstream_metrics.data_bytes_processed += bytes_processed as u64;
        metrics.shredstream_metrics.last_data_received = Some(Utc::now());

        // Update latency metrics
        if latency_ms > 0.0 {
            let current_avg = metrics.shredstream_metrics.average_latency_ms;
            let count = metrics.shredstream_metrics.data_bytes_received;
            metrics.shredstream_metrics.average_latency_ms =
                (current_avg * (count - 1) as f64 + latency_ms) / count as f64;

            if latency_ms > metrics.shredstream_metrics.peak_latency_ms {
                metrics.shredstream_metrics.peak_latency_ms = latency_ms;
            }
        }
    }

    /// Record protobuf parsing result
    pub fn record_protobuf_parsing(&self, messages_parsed: usize, failures: usize) {
        let mut metrics = self.metrics.write();
        metrics.shredstream_metrics.protobuf_messages_parsed += messages_parsed as u64;
        metrics.shredstream_metrics.protobuf_parse_failures += failures as u64;
    }

    /// Record arbitrage opportunity detection
    pub fn record_arbitrage_opportunity(&self, opportunity_type: &str) {
        let mut metrics = self.metrics.write();
        metrics.arbitrage_metrics.opportunities_detected += 1;

        match opportunity_type {
            "cross_dex" => metrics.arbitrage_metrics.cross_dex_opportunities += 1,
            "triangular" => metrics.arbitrage_metrics.triangular_opportunities += 1,
            "sandwich" => metrics.arbitrage_metrics.sandwich_opportunities += 1,
            _ => {}
        }
    }

    /// Record arbitrage execution result
    pub fn record_arbitrage_execution(&self, successful: bool, profit_sol: f64, fees_sol: f64, execution_time_ms: f64) {
        let mut metrics = self.metrics.write();

        if successful {
            metrics.arbitrage_metrics.opportunities_executed += 1;
            if profit_sol > 0.0 {
                metrics.arbitrage_metrics.profitable_trades += 1;
            } else {
                metrics.arbitrage_metrics.losing_trades += 1;
            }
        } else {
            metrics.arbitrage_metrics.opportunities_failed += 1;
        }

        metrics.arbitrage_metrics.total_profit_sol += profit_sol;
        metrics.arbitrage_metrics.total_fees_paid_sol += fees_sol;

        // Update execution time metrics
        let current_avg = metrics.arbitrage_metrics.average_execution_time_ms;
        let count = metrics.arbitrage_metrics.opportunities_executed;
        if count > 0 {
            metrics.arbitrage_metrics.average_execution_time_ms =
                (current_avg * (count - 1) as f64 + execution_time_ms) / count as f64;

            if execution_time_ms > metrics.arbitrage_metrics.peak_execution_time_ms {
                metrics.arbitrage_metrics.peak_execution_time_ms = execution_time_ms;
            }
        }
    }

    /// Record MEV protection activation
    pub fn record_mev_protection(&self) {
        let mut metrics = self.metrics.write();
        metrics.arbitrage_metrics.mev_protection_activations += 1;
    }

    /// Record network request
    pub fn record_network_request(&self, request_type: &str, successful: bool, latency_ms: f64) {
        let mut metrics = self.metrics.write();

        match request_type {
            "rpc" => {
                metrics.network_metrics.rpc_requests_total += 1;
                if successful {
                    metrics.network_metrics.rpc_requests_successful += 1;
                } else {
                    metrics.network_metrics.rpc_requests_failed += 1;
                }

                // Update RPC latency
                let current_avg = metrics.network_metrics.average_rpc_latency_ms;
                let count = metrics.network_metrics.rpc_requests_total;
                metrics.network_metrics.average_rpc_latency_ms =
                    (current_avg * (count - 1) as f64 + latency_ms) / count as f64;

                if latency_ms > metrics.network_metrics.peak_rpc_latency_ms {
                    metrics.network_metrics.peak_rpc_latency_ms = latency_ms;
                }
            }
            "jupiter" => {
                metrics.network_metrics.jupiter_api_calls += 1;
                if !successful {
                    metrics.network_metrics.jupiter_api_failures += 1;
                }
            }
            "jito" => {
                metrics.network_metrics.jito_bundle_submissions += 1;
                if successful {
                    metrics.network_metrics.jito_bundle_successes += 1;
                }
            }
            _ => {}
        }
    }

    /// Update system resource metrics
    pub fn update_system_metrics(&self, cpu_percent: f64, memory_mb: f64, memory_percent: f64) {
        let mut metrics = self.metrics.write();
        metrics.system_metrics.cpu_usage_percent = cpu_percent;
        metrics.system_metrics.memory_usage_mb = memory_mb;
        metrics.system_metrics.memory_usage_percent = memory_percent;
    }

    /// Update uptime
    pub fn update_uptime(&self) {
        let mut metrics = self.metrics.write();
        metrics.shredstream_metrics.uptime_seconds = self.start_time.elapsed().as_secs();
    }

    /// Get current metrics snapshot
    pub fn get_metrics(&self) -> PerformanceMetrics {
        self.metrics.read().clone()
    }

    /// Generate comprehensive performance report
    pub fn generate_performance_report(&self) -> PerformanceReport {
        let metrics = self.metrics.read();
        let uptime_hours = metrics.shredstream_metrics.uptime_seconds as f64 / 3600.0;

        PerformanceReport {
            timestamp: Utc::now(),
            uptime_hours,
            summary: format!(
                "Uptime: {:.1}h | Opportunities: {} | Executed: {} | Profit: {:.4} SOL",
                uptime_hours,
                metrics.arbitrage_metrics.opportunities_detected,
                metrics.arbitrage_metrics.opportunities_executed,
                metrics.arbitrage_metrics.total_profit_sol
            ),
            shredstream_health: self.calculate_shredstream_health(&metrics),
            arbitrage_performance: self.calculate_arbitrage_performance(&metrics),
            system_health: self.calculate_system_health(&metrics),
            network_health: self.calculate_network_health(&metrics),
            trading_performance: self.calculate_trading_performance(&metrics),
            recommendations: self.generate_recommendations(&metrics),
        }
    }

    /// Calculate ShredStream health score (0-100)
    fn calculate_shredstream_health(&self, metrics: &PerformanceMetrics) -> f64 {
        let mut score = 100.0;

        // Penalize connection failures
        if metrics.shredstream_metrics.connections_established > 0 {
            let failure_rate = metrics.shredstream_metrics.connection_failures as f64
                / metrics.shredstream_metrics.connections_established as f64;
            score -= failure_rate * 30.0;
        }

        // Penalize circuit breaker opens
        score -= metrics.shredstream_metrics.circuit_breaker_opens as f64 * 10.0;

        // Penalize high latency
        if metrics.shredstream_metrics.average_latency_ms > 10.0 {
            score -= (metrics.shredstream_metrics.average_latency_ms - 10.0) * 2.0;
        }

        score.max(0.0).min(100.0)
    }

    /// Calculate arbitrage performance score (0-100)
    fn calculate_arbitrage_performance(&self, metrics: &PerformanceMetrics) -> f64 {
        if metrics.arbitrage_metrics.opportunities_detected == 0 {
            return 50.0; // Neutral score if no opportunities
        }

        let execution_rate = metrics.arbitrage_metrics.opportunities_executed as f64
            / metrics.arbitrage_metrics.opportunities_detected as f64;

        let win_rate = if metrics.arbitrage_metrics.opportunities_executed > 0 {
            metrics.arbitrage_metrics.profitable_trades as f64
                / metrics.arbitrage_metrics.opportunities_executed as f64
        } else {
            0.0
        };

        let mut score = execution_rate * 40.0 + win_rate * 60.0;

        // Bonus for profitability
        if metrics.arbitrage_metrics.total_profit_sol > 0.0 {
            score += 10.0;
        }

        score.max(0.0).min(100.0)
    }

    /// Calculate system health score (0-100)
    fn calculate_system_health(&self, metrics: &PerformanceMetrics) -> f64 {
        let mut score = 100.0;

        // CPU usage penalty
        if metrics.system_metrics.cpu_usage_percent > 80.0 {
            score -= metrics.system_metrics.cpu_usage_percent - 80.0;
        }

        // Memory usage penalty
        if metrics.system_metrics.memory_usage_percent > 80.0 {
            score -= metrics.system_metrics.memory_usage_percent - 80.0;
        }

        score.max(0.0).min(100.0)
    }

    /// Calculate network health score (0-100)
    fn calculate_network_health(&self, metrics: &PerformanceMetrics) -> f64 {
        let mut score = 100.0;

        // RPC failure rate penalty
        if metrics.network_metrics.rpc_requests_total > 0 {
            let failure_rate = metrics.network_metrics.rpc_requests_failed as f64
                / metrics.network_metrics.rpc_requests_total as f64;
            score -= failure_rate * 40.0;
        }

        // High latency penalty
        if metrics.network_metrics.average_rpc_latency_ms > 500.0 {
            score -= (metrics.network_metrics.average_rpc_latency_ms - 500.0) / 10.0;
        }

        score.max(0.0).min(100.0)
    }

    /// Calculate trading performance score (0-100)
    fn calculate_trading_performance(&self, metrics: &PerformanceMetrics) -> f64 {
        if metrics.trading_metrics.positions_closed == 0 {
            return 50.0;
        }

        let mut score = metrics.trading_metrics.win_rate_percent * 0.6;

        if metrics.trading_metrics.profit_factor > 1.0 {
            score += 20.0;
        }

        if metrics.trading_metrics.sharpe_ratio > 1.0 {
            score += 20.0;
        }

        score.max(0.0).min(100.0)
    }

    /// Generate performance recommendations
    fn generate_recommendations(&self, metrics: &PerformanceMetrics) -> Vec<String> {
        let mut recommendations = Vec::new();

        // ShredStream recommendations
        if metrics.shredstream_metrics.average_latency_ms > 10.0 {
            recommendations.push("Consider optimizing ShredStream connection or switching endpoints".to_string());
        }

        if metrics.shredstream_metrics.circuit_breaker_opens > 5 {
            recommendations.push("Investigate frequent circuit breaker activations".to_string());
        }

        // System recommendations
        if metrics.system_metrics.cpu_usage_percent > 80.0 {
            recommendations.push("High CPU usage detected - consider optimizing algorithms".to_string());
        }

        if metrics.system_metrics.memory_usage_percent > 80.0 {
            recommendations.push("High memory usage detected - review memory management".to_string());
        }

        // Trading recommendations
        if metrics.arbitrage_metrics.opportunities_executed > 0 {
            let execution_rate = metrics.arbitrage_metrics.opportunities_executed as f64
                / metrics.arbitrage_metrics.opportunities_detected as f64;

            if execution_rate < 0.1 {
                recommendations.push("Low execution rate - review opportunity filtering criteria".to_string());
            }
        }

        recommendations
    }
}

/// Comprehensive performance report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceReport {
    pub timestamp: DateTime<Utc>,
    pub uptime_hours: f64,
    pub summary: String,
    pub shredstream_health: f64,
    pub arbitrage_performance: f64,
    pub system_health: f64,
    pub network_health: f64,
    pub trading_performance: f64,
    pub recommendations: Vec<String>,
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}