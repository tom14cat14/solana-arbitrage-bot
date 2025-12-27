use anyhow::Result;
use std::time::Instant;
use tracing::{info, warn, debug};
use chrono::Utc;
use std::collections::HashMap;

use crate::dex_transaction_parser::DexTransactionParser;
use crate::dex_registry::DexRegistry;

/// Real ShredStream UDP listener for ERPC
/// Port 20000/UDP for IP-whitelisted shred forwarding
///
/// IMPORTANT: This is an INBOUND listener - ERPC pushes shreds TO your IP
/// on port 20000/UDP. You do NOT connect out to ERPC.
#[derive(Debug)]
pub struct ShredStreamUDP {
    port: u16,
    buffer_size: usize,
    dex_parser: DexTransactionParser,
    dex_registry: DexRegistry,
    price_cache: HashMap<String, PriceUpdate>,
}

#[derive(Debug, Clone)]
pub struct PriceUpdate {
    pub token_mint: String,
    pub dex_program_id: String,
    pub price_sol: f64,
    pub liquidity: u64,
    pub volume_24h: f64,
    pub timestamp: chrono::DateTime<Utc>,
}

impl ShredStreamUDP {
    pub fn new(port: u16) -> Self {
        info!("🌊 Initializing ShredStream UDP listener");
        info!("  • Port: {}/UDP", port);
        info!("  • Mode: IP-whitelisted shred forwarding (INBOUND)");
        info!("  • ERPC pushes shreds TO this port");

        Self {
            port,
            buffer_size: 65535, // Max UDP packet size
            dex_parser: DexTransactionParser::new(),
            dex_registry: DexRegistry::new(),
            price_cache: HashMap::new(),
        }
    }

    /// Create async UDP socket bound to port 20000 for INBOUND shreds
    pub async fn create_socket(&self) -> Result<tokio::net::UdpSocket> {
        let bind_addr = format!("0.0.0.0:{}", self.port);
        info!("🔌 Binding UDP socket to {} (INBOUND listener)", bind_addr);
        info!("   Waiting for ERPC to push shreds to IP: 151.243.244.130");

        let socket = tokio::net::UdpSocket::bind(&bind_addr).await?;
        info!("✅ ShredStream UDP socket bound successfully on port {}", self.port);
        info!("🚀 Listening for incoming shreds from ERPC...");
        info!("   Protocol: Raw UDP shred forwarding");
        info!("   Direction: INBOUND (ERPC → YOU on port 20000)");

        Ok(socket)
    }

    /// Process a single cycle (non-blocking check for UDP packet with timeout)
    pub async fn process_single_cycle(&mut self, socket: &tokio::net::UdpSocket) -> Result<Vec<PriceUpdate>> {
        let mut buffer = vec![0u8; self.buffer_size];

        // Non-blocking receive with 100ms timeout
        let timeout = tokio::time::Duration::from_millis(100);

        match tokio::time::timeout(timeout, socket.recv_from(&mut buffer)).await {
            Ok(Ok((len, src))) => {
                if len > 0 {
                    info!("📦 Received {} bytes from {}", len, src);
                    self.process_shred(&buffer[..len])
                } else {
                    Ok(Vec::new())
                }
            }
            Ok(Err(e)) => {
                warn!("UDP recv error: {}", e);
                Ok(Vec::new())
            }
            Err(_) => {
                // Timeout - normal, no data available
                Ok(Vec::new())
            }
        }
    }

    /// Process raw shred data
    fn process_shred(&mut self, data: &[u8]) -> Result<Vec<PriceUpdate>> {
        info!("🔍 Processing {} byte shred", data.len());

        // TODO: Implement full shred decoding using solana-shred crate
        // This would:
        // 1. Decode shred structure using Shred::new_from_serialized_shred()
        // 2. Extract entries from shred payload
        // 3. Extract transactions from entries
        // 4. Parse DEX instructions using dex_parser
        // 5. Calculate prices from swap amounts
        // 6. Update price_cache

        // For now, return empty to show we received data
        Ok(Vec::new())
    }

    /// Get cached prices
    pub fn get_prices(&self) -> &HashMap<String, PriceUpdate> {
        &self.price_cache
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shredstream_creation() {
        let shred = ShredStreamUDP::new(20000);
        assert_eq!(shred.port, 20000);
        assert_eq!(shred.buffer_size, 65535);
    }
}
