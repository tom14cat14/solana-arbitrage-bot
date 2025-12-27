use anyhow::Result;
use futures::StreamExt;
use solana_stream_sdk::{CommitmentLevel, ShredstreamClient};
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let endpoint = "https://shreds-ny6-1.erpc.global";
    info!("🌊 Connecting to ShredStream: {}", endpoint);

    let mut client = ShredstreamClient::connect(endpoint).await?;
    info!("✅ Connected successfully!");

    let request = ShredstreamClient::create_entries_request_for_accounts(
        vec![],
        vec![],
        vec![],
        Some(CommitmentLevel::Processed),
    );

    info!("📡 Subscribing to entries...");
    let mut stream = client.subscribe_entries(request).await?;
    info!("✅ Subscription active - receiving shreds!");

    let mut count = 0;
    while let Some(slot_entry) = stream.next().await {
        match slot_entry {
            Ok(data) => {
                count += 1;
                if let Ok(entries) = bincode::deserialize::<Vec<solana_entry::entry::Entry>>(&data.entries) {
                    let tx_count: usize = entries.iter().map(|e| e.transactions.len()).sum();
                    info!(
                        "📦 Slot {}, entries: {}, transactions: {}",
                        data.slot,
                        entries.len(),
                        tx_count
                    );
                }

                if count >= 10 {
                    info!("✅ Test successful - received {} shred packets", count);
                    break;
                }
            }
            Err(e) => {
                info!("❌ Error: {:?}", e);
                break;
            }
        }
    }

    Ok(())
}
