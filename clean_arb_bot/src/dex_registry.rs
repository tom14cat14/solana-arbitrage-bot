use std::collections::HashMap;

/// Information about a DEX
#[derive(Debug, Clone)]
pub struct DexInfo {
    pub name: String,
    pub program_id: String,
    pub fee_rate: f64,
    pub supports_arbitrage: bool,
    pub min_liquidity_threshold: u64,
}

/// Registry of all supported DEXs
#[derive(Debug, Clone)]
pub struct DexRegistry {
    dexs: HashMap<String, DexInfo>,
}

impl DexRegistry {
    pub fn new() -> Self {
        let mut dexs = HashMap::new();

        // DEX configurations: (name, program_id, fee_rate, supports_arb, min_liquidity)
        let configs = vec![
            // Raydium
            ("Raydium", "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8", 0.0025, true, 1_000_000),
            ("Raydium_CLMM", "CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK", 0.0025, true, 1_000_000),
            ("Raydium_CPMM", "CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C", 0.0025, true, 1_000_000),

            // Orca
            ("Orca", "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc", 0.003, true, 5_000_000),

            // Jupiter
            ("Jupiter", "JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4", 0.001, true, 0),

            // Meteora
            ("Meteora", "LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo", 0.003, true, 2_000_000),

            // Serum
            ("Serum", "9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin", 0.0022, true, 10_000_000),

            // PumpSwap (migrated tokens)
            ("PumpSwap", "GMk6j2defJhS7F194toqmJNFNhAkbDXhYJo5oR3Rpump", 0.003, true, 100_000),

            // Others
            ("Aldrin", "AMM55ShdkoGRB5jVYPjWziwk8m5MpwyDgsMWHaMSQWH6", 0.003, true, 1_000_000),
            ("Lifinity", "EewxydAPCCVuNEyrVN68PuSYdQ7wKn27V9Gjeoi8dy3S", 0.0025, true, 2_000_000),
            ("Crema", "6MLxLqiXaaSUpkgMnWDTuejNZEz3kE7k2woyHGVFw319", 0.003, true, 1_000_000),
        ];

        for (name, program_id, fee_rate, supports_arb, min_liquidity) in configs {
            if supports_arb {
                dexs.insert(name.to_string(), DexInfo {
                    name: name.to_string(),
                    program_id: program_id.to_string(),
                    fee_rate,
                    supports_arbitrage: supports_arb,
                    min_liquidity_threshold: min_liquidity,
                });
            }
        }

        Self { dexs }
    }

    pub fn get_all_dexs(&self) -> &HashMap<String, DexInfo> {
        &self.dexs
    }

    pub fn get_dex(&self, name: &str) -> Option<&DexInfo> {
        self.dexs.get(name)
    }

    /// Get all DEX pairs for arbitrage scanning
    pub fn get_arbitrage_pairs(&self) -> Vec<(&DexInfo, &DexInfo)> {
        let dex_list: Vec<&DexInfo> = self.dexs.values().collect();
        let mut pairs = Vec::new();

        for i in 0..dex_list.len() {
            for j in (i + 1)..dex_list.len() {
                pairs.push((dex_list[i], dex_list[j]));
            }
        }

        pairs
    }
}
