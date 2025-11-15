// Dynamic JITO Tip Floor Monitor
//
// Monitors JITO's tip floor API every 30 minutes to adjust tips competitively
// without overpaying. Uses percentile data to beat 95-99% of market.

use anyhow::Result;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};
use tracing::{debug, error, info, warn};

/// JITO tip floor percentile data from their API
#[derive(Debug, Clone, Deserialize)]
pub struct JitoTipFloor {
    #[serde(rename = "landed_tips_25th_percentile")]
    pub p25: f64, // 25th percentile in SOL

    #[serde(rename = "landed_tips_50th_percentile")]
    pub p50: f64, // 50th percentile in SOL

    #[serde(rename = "landed_tips_75th_percentile")]
    pub p75: f64, // 75th percentile in SOL

    #[serde(rename = "landed_tips_95th_percentile")]
    pub p95: f64, // 95th percentile in SOL

    #[serde(rename = "landed_tips_99th_percentile")]
    pub p99: f64, // 99th percentile in SOL

    #[serde(rename = "ema_landed_tips_50th_percentile")]
    pub ema_p50: f64, // Exponential moving average of 50th percentile

    /// When this data was last updated (not deserialized, set manually)
    #[serde(skip, default = "std::time::Instant::now")]
    pub last_updated: std::time::Instant,
}

impl Default for JitoTipFloor {
    fn default() -> Self {
        Self {
            // Conservative defaults (if API fails, use higher tips)
            p25: 0.000001,  // 1,000 lamports
            p50: 0.000001,  // 1,000 lamports
            p75: 0.000010,  // 10,000 lamports
            p95: 0.001000,  // 1M lamports (conservative)
            p99: 0.010000,  // 10M lamports (conservative)
            ema_p50: 0.000001,
            last_updated: std::time::Instant::now(),
        }
    }
}

impl JitoTipFloor {
    /// Convert SOL amounts to lamports
    pub fn p95_lamports(&self) -> u64 {
        (self.p95 * 1_000_000_000.0) as u64
    }

    pub fn p99_lamports(&self) -> u64 {
        (self.p99 * 1_000_000_000.0) as u64
    }

    /// Get competitive tip: 10% above percentile to beat competition
    /// HARD CAP: Maximum 0.003 SOL (3M lamports) to prevent extreme market spikes
    pub fn competitive_tip_95(&self) -> u64 {
        const MAX_TIP: u64 = 3_000_000; // 0.003 SOL hard cap
        let tip = (self.p95_lamports() as f64 * 1.10) as u64;
        let capped_tip = tip.min(MAX_TIP);

        if capped_tip < tip {
            debug!("🔒 95th percentile tip CAPPED: {:.6} SOL → {:.6} SOL (market spike protection)",
                   tip as f64 / 1e9, capped_tip as f64 / 1e9);
        }

        capped_tip
    }

    pub fn competitive_tip_99(&self) -> u64 {
        const MAX_TIP: u64 = 3_000_000; // 0.003 SOL hard cap
        let tip = (self.p99_lamports() as f64 * 1.10) as u64;
        let capped_tip = tip.min(MAX_TIP);

        if capped_tip < tip {
            debug!("🔒 99th percentile tip CAPPED: {:.6} SOL → {:.6} SOL (market spike protection)",
                   tip as f64 / 1e9, capped_tip as f64 / 1e9);
        }

        capped_tip
    }

    /// Check if data is stale (>15 minutes old - 5 min buffer)
    pub fn is_stale(&self) -> bool {
        self.last_updated.elapsed() > Duration::from_secs(15 * 60)
    }
}

/// Shared JITO tip floor data (thread-safe)
pub type SharedJitoTipFloor = Arc<RwLock<JitoTipFloor>>;

/// API response from JITO tip floor endpoint
#[derive(Debug, Deserialize)]
struct JitoTipFloorResponse {
    time: String,
    landed_tips_25th_percentile: f64,
    landed_tips_50th_percentile: f64,
    landed_tips_75th_percentile: f64,
    landed_tips_95th_percentile: f64,
    landed_tips_99th_percentile: f64,
    ema_landed_tips_50th_percentile: f64,
}

/// Fetch current JITO tip floor data from API
async fn fetch_jito_tip_floor() -> Result<JitoTipFloor> {
    let url = "https://bundles.jito.wtf/api/v1/bundles/tip_floor";

    let response = reqwest::get(url).await?;

    if !response.status().is_success() {
        anyhow::bail!("JITO tip floor API returned {}", response.status());
    }

    let data: Vec<JitoTipFloorResponse> = response.json().await?;

    // Get the most recent entry (first in array)
    let latest = data
        .first()
        .ok_or_else(|| anyhow::anyhow!("Empty response from JITO tip floor API"))?;

    Ok(JitoTipFloor {
        p25: latest.landed_tips_25th_percentile,
        p50: latest.landed_tips_50th_percentile,
        p75: latest.landed_tips_75th_percentile,
        p95: latest.landed_tips_95th_percentile,
        p99: latest.landed_tips_99th_percentile,
        ema_p50: latest.ema_landed_tips_50th_percentile,
        last_updated: std::time::Instant::now(),
    })
}

/// Background task that monitors JITO tip floor every 30 minutes
///
/// # Arguments
/// * `tip_floor` - Shared tip floor data (updated by this task)
///
/// # Behavior
/// - Fetches JITO tip floor data every 30 minutes
/// - Updates shared state with latest percentiles
/// - Logs percentile changes for monitoring
/// - Retries on failure with exponential backoff
pub async fn monitor_jito_tip_floor(tip_floor: SharedJitoTipFloor) {
    info!("🚀 JITO tip floor monitor started (updates every 10 minutes)");

    // Initial fetch on startup
    match fetch_jito_tip_floor().await {
        Ok(data) => {
            info!("📊 Initial JITO tip floor:");
            info!("   50th percentile: {:.9} SOL ({} lamports)", data.p50, (data.p50 * 1e9) as u64);
            info!("   95th percentile: {:.9} SOL ({} lamports)", data.p95, data.p95_lamports());
            info!("   99th percentile: {:.9} SOL ({} lamports)", data.p99, data.p99_lamports());
            *tip_floor.write().await = data;
        }
        Err(e) => {
            warn!("⚠️  Failed to fetch initial JITO tip floor: {}", e);
            warn!("   Using conservative defaults until next fetch");
        }
    }

    // Monitor loop - update every 10 minutes
    let mut retry_delay = Duration::from_secs(10 * 60); // 10 minutes

    loop {
        sleep(retry_delay).await;

        match fetch_jito_tip_floor().await {
            Ok(new_data) => {
                let old_data = tip_floor.read().await.clone();

                // Log significant changes (>20% difference)
                let p95_change = (new_data.p95 - old_data.p95) / old_data.p95 * 100.0;
                let p99_change = (new_data.p99 - old_data.p99) / old_data.p99 * 100.0;

                if p95_change.abs() > 20.0 || p99_change.abs() > 20.0 {
                    info!("📊 JITO tip floor changed significantly:");
                    info!("   95th: {:.9} SOL → {:.9} SOL ({:+.1}%)",
                          old_data.p95, new_data.p95, p95_change);
                    info!("   99th: {:.9} SOL → {:.9} SOL ({:+.1}%)",
                          old_data.p99, new_data.p99, p99_change);
                } else {
                    debug!("✅ JITO tip floor updated (no major changes)");
                }

                *tip_floor.write().await = new_data;

                // Reset to 10 minute interval on success
                retry_delay = Duration::from_secs(10 * 60);
            }
            Err(e) => {
                error!("❌ Failed to fetch JITO tip floor: {}", e);

                // Exponential backoff on failure (up to 10 minutes)
                retry_delay = Duration::from_secs((retry_delay.as_secs() * 2).min(10 * 60));
                warn!("   Retrying in {} minutes", retry_delay.as_secs() / 60);

                // Check if data is getting stale
                let current_data = tip_floor.read().await;
                if current_data.is_stale() {
                    warn!("⚠️  JITO tip floor data is >35 minutes old!");
                    warn!("   Using stale data (better than defaults)");
                }
            }
        }
    }
}

/// Spawn JITO tip floor monitor as background task
///
/// # Returns
/// Shared tip floor data that will be updated every 30 minutes
pub fn spawn_monitor() -> SharedJitoTipFloor {
    let tip_floor = Arc::new(RwLock::new(JitoTipFloor::default()));
    let tip_floor_clone = tip_floor.clone();

    tokio::spawn(async move {
        monitor_jito_tip_floor(tip_floor_clone).await;
    });

    tip_floor
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_tip_floor() {
        let floor = JitoTipFloor::default();
        assert!(floor.p95_lamports() > 0);
        assert!(floor.p99_lamports() > floor.p95_lamports());
    }

    #[test]
    fn test_competitive_tips() {
        let floor = JitoTipFloor {
            p95: 0.001,  // 1M lamports
            p99: 0.010,  // 10M lamports
            ..Default::default()
        };

        // Competitive tips should be 10% above percentile
        assert_eq!(floor.competitive_tip_95(), 1_100_000); // 1.1M lamports
        assert_eq!(floor.competitive_tip_99(), 3_000_000); // Capped at 3M (would be 11M without cap)
    }

    #[test]
    fn test_hard_cap() {
        // Test that extreme market spikes are capped at 0.003 SOL
        let extreme_floor = JitoTipFloor {
            p95: 0.010,  // 10M lamports (extreme)
            p99: 0.100,  // 100M lamports (extreme spike)
            ..Default::default()
        };

        // Both should be capped at 3M lamports (0.003 SOL)
        assert_eq!(extreme_floor.competitive_tip_95(), 3_000_000); // Capped (would be 11M)
        assert_eq!(extreme_floor.competitive_tip_99(), 3_000_000); // Capped (would be 110M)
    }

    #[tokio::test]
    async fn test_fetch_jito_tip_floor() {
        // This test requires network access
        match fetch_jito_tip_floor().await {
            Ok(data) => {
                println!("✅ Fetched JITO tip floor:");
                println!("   95th: {:.9} SOL", data.p95);
                println!("   99th: {:.9} SOL", data.p99);
                assert!(data.p99 >= data.p95);
            }
            Err(e) => {
                println!("⚠️  Failed to fetch (may be offline): {}", e);
            }
        }
    }
}
