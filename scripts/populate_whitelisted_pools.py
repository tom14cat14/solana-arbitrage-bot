#!/usr/bin/env python3
"""
Populate All Pools for Whitelisted Tokens
Queries GeckoTerminal for ALL pools (active and stale) for each whitelisted token
"""

import requests
import json
import time
from typing import List, Dict

# Load target tokens
with open("/home/tom14cat14/Arb_Bot/target_tokens.json", "r") as f:
    target_data = json.load(f)
    target_tokens = [t["address"] for t in target_data["tokens"]]

print(f"🔍 Loading pools for {len(target_tokens)} whitelisted tokens...")

all_pools = {}
failed_tokens = []

for i, token_address in enumerate(target_tokens):
    if i % 10 == 0:
        print(f"   Progress: {i}/{len(target_tokens)} tokens...")

    try:
        # Query GeckoTerminal for all pools
        url = f"https://api.geckoterminal.com/api/v2/networks/solana/tokens/{token_address}/pools"
        response = requests.get(url, timeout=10)

        if response.status_code == 200:
            data = response.json()
            pools = data.get("data", [])

            if pools:
                all_pools[token_address] = [
                    {
                        "pool_address": pool["attributes"]["address"],
                        "dex": pool["attributes"].get("dex_id", "unknown"),
                        "liquidity_usd": float(pool["attributes"].get("reserve_in_usd", 0)),
                        "volume_24h_usd": float(pool["attributes"].get("volume_usd", {}).get("h24", 0)),
                        "price_usd": float(pool["attributes"].get("base_token_price_usd", 0)),
                    }
                    for pool in pools
                ]
                print(f"   ✅ {token_address[:8]}: {len(pools)} pools")
        else:
            failed_tokens.append(token_address)

        # Rate limiting
        time.sleep(0.3)  # ~3 requests/sec

    except Exception as e:
        print(f"   ⚠️ Failed {token_address[:8]}: {e}")
        failed_tokens.append(token_address)

print(f"\n✅ Found pools for {len(all_pools)} tokens")
print(f"❌ Failed: {len(failed_tokens)} tokens")

# Save results
output = {
    "generated_at": time.strftime("%Y-%m-%d %H:%M:%S"),
    "total_tokens": len(target_tokens),
    "tokens_with_pools": len(all_pools),
    "total_pools": sum(len(pools) for pools in all_pools.values()),
    "pools": all_pools
}

with open("/home/tom14cat14/Arb_Bot/whitelisted_pools.json", "w") as f:
    json.dump(output, f, indent=2)

print(f"\n💾 Saved to: /home/tom14cat14/Arb_Bot/whitelisted_pools.json")
print(f"📊 Total pools: {output['total_pools']}")
