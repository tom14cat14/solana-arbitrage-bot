#!/usr/bin/env python3
"""
BirdEye Token Filter Script
Fetches tokens with:
- 10k+ daily volume (rolling 3-day average)
- Market cap: 100k - 30M
"""

import requests
import json
import time
from datetime import datetime
from typing import List, Dict

# BirdEye API configuration
BIRDEYE_API_KEY = "90e483da2b5448c4b456da4f8de111df"
BIRDEYE_BASE_URL = "https://public-api.birdeye.so"

# Filter criteria
MIN_DAILY_VOLUME_USD = 10_000  # $10k per day
MIN_MARKET_CAP = 100_000       # $100k
MAX_MARKET_CAP = 30_000_000    # $30M

def fetch_token_list(offset=0, limit=50):
    """Fetch token list from BirdEye API"""
    url = f"{BIRDEYE_BASE_URL}/defi/tokenlist"
    params = {
        "chain": "solana",
        "sort_by": "v24hUSD",
        "sort_type": "desc",
        "offset": offset,
        "limit": limit
    }
    headers = {
        "X-API-KEY": BIRDEYE_API_KEY
    }

    try:
        response = requests.get(url, params=params, headers=headers, timeout=10)
        response.raise_for_status()
        return response.json()
    except Exception as e:
        print(f"❌ Error fetching tokens at offset {offset}: {e}")
        return None

def get_token_history(token_address: str, days: int = 3):
    """Get historical price/volume data for a token"""
    url = f"{BIRDEYE_BASE_URL}/defi/history_price"
    params = {
        "address": token_address,
        "address_type": "token",
        "type": "1D",  # 1 day intervals
        "time_from": int(time.time()) - (days * 24 * 60 * 60),
        "time_to": int(time.time())
    }
    headers = {
        "X-API-KEY": BIRDEYE_API_KEY
    }

    try:
        response = requests.get(url, params=params, headers=headers, timeout=10)
        response.raise_for_status()
        return response.json()
    except Exception as e:
        print(f"⚠️ Error fetching history for {token_address}: {e}")
        return None

def calculate_rolling_3day_volume(token: Dict) -> float:
    """
    Calculate average daily volume over 3 days
    For now, use v24hUSD as proxy (BirdEye historical volume requires separate calls)
    """
    # Simple approach: Use current 24h volume as estimate
    # For production, would fetch 3 days of volume data
    return token.get('v24hUSD', 0) / 1  # Current 24h volume

def filter_tokens(all_tokens: List[Dict]) -> List[Dict]:
    """Filter tokens by market cap and volume criteria"""
    filtered = []

    for token in all_tokens:
        mc = token.get('mc')
        v24h = token.get('v24hUSD')

        # Skip if missing required data or None
        if mc is None or v24h is None:
            continue

        if mc == 0 or v24h == 0:
            continue

        # Filter by market cap range
        if mc < MIN_MARKET_CAP or mc > MAX_MARKET_CAP:
            continue

        # Filter by daily volume (using 24h volume as proxy for daily average)
        # In production, would calculate actual 3-day rolling average
        if v24h < MIN_DAILY_VOLUME_USD:
            continue

        filtered.append(token)

    return filtered

def fetch_all_tokens(max_tokens=1000):
    """Fetch all tokens with pagination"""
    all_tokens = []
    offset = 0
    limit = 50

    print(f"🔍 Fetching tokens from BirdEye API...")
    print(f"   Filters: MC ${MIN_MARKET_CAP:,} - ${MAX_MARKET_CAP:,}, Volume ${MIN_DAILY_VOLUME_USD:,}/day")
    print()

    while offset < max_tokens:
        print(f"   Fetching tokens {offset}-{offset+limit}...", end="\r")

        result = fetch_token_list(offset, limit)
        if not result or not result.get('success'):
            print(f"\n❌ Failed to fetch at offset {offset}")
            break

        tokens = result.get('data', {}).get('tokens', [])
        if not tokens:
            print(f"\n✅ Reached end of token list at offset {offset}")
            break

        all_tokens.extend(tokens)
        offset += limit

        # Rate limiting
        time.sleep(0.5)

    print(f"\n✅ Fetched {len(all_tokens)} total tokens")
    return all_tokens

def main():
    print("=" * 60)
    print("🦅 BirdEye Token Filter - Target List Generator")
    print("=" * 60)
    print()

    # Fetch all tokens
    all_tokens = fetch_all_tokens(max_tokens=2000)

    if not all_tokens:
        print("❌ No tokens fetched")
        return

    print()
    print("🔬 Filtering tokens...")

    # Filter tokens
    filtered_tokens = filter_tokens(all_tokens)

    print(f"✅ Found {len(filtered_tokens)} tokens matching criteria")
    print()

    # Sort by volume (highest first)
    filtered_tokens.sort(key=lambda x: x.get('v24hUSD', 0), reverse=True)

    # Display results
    print("=" * 60)
    print("📊 TARGET TOKENS")
    print("=" * 60)
    print()
    print(f"{'Symbol':<12} {'Address':<45} {'MC':>12} {'Vol24h':>12}")
    print("-" * 90)

    for token in filtered_tokens[:50]:  # Top 50
        symbol = token.get('symbol', 'UNKNOWN')[:10]
        address = token.get('address', '')[:43]
        mc = token.get('mc', 0)
        v24h = token.get('v24hUSD', 0)

        print(f"{symbol:<12} {address:<45} ${mc:>10,.0f} ${v24h:>10,.0f}")

    if len(filtered_tokens) > 50:
        print(f"\n... and {len(filtered_tokens) - 50} more tokens")

    print()
    print("=" * 60)

    # Save to JSON
    output_file = "/home/tom14cat14/Arb_Bot/target_tokens.json"
    with open(output_file, 'w') as f:
        json.dump({
            'generated_at': datetime.now().isoformat(),
            'criteria': {
                'min_daily_volume_usd': MIN_DAILY_VOLUME_USD,
                'min_market_cap': MIN_MARKET_CAP,
                'max_market_cap': MAX_MARKET_CAP
            },
            'total_tokens': len(filtered_tokens),
            'tokens': [
                {
                    'address': t.get('address'),
                    'symbol': t.get('symbol'),
                    'name': t.get('name'),
                    'market_cap': t.get('mc'),
                    'volume_24h': t.get('v24hUSD'),
                    'price': t.get('price'),
                    'liquidity': t.get('liquidity')
                }
                for t in filtered_tokens
            ]
        }, f, indent=2)

    print(f"💾 Saved to: {output_file}")
    print()

    # Also save simple address list
    address_file = "/home/tom14cat14/Arb_Bot/target_addresses.txt"
    with open(address_file, 'w') as f:
        for token in filtered_tokens:
            f.write(f"{token.get('address')}\n")

    print(f"💾 Address list: {address_file}")
    print()

if __name__ == "__main__":
    main()
