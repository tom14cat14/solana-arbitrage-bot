//! CYCLE-7: Slippage Protection Module
//! Implements dynamic slippage calculations based on historical volatility
//! Grok recommendation for "bulletproof" live trading

use rust_decimal::Decimal;
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};

/// Calculate expected slippage based on market price and volatility
///
/// # Arguments
/// * `expected_price` - The price we expect to execute at
/// * `market_price` - The current market price
/// * `volatility` - Historical volatility percentage (e.g., 5.0 for 5%)
///
/// # Returns
/// * Decimal value representing expected slippage amount
///
/// # Example
/// ```
/// use rust_decimal::Decimal;
/// let expected = Decimal::from_f64(1.0).unwrap();
/// let market = Decimal::from_f64(1.01).unwrap();
/// let slippage = calculate_slippage(expected, market, 5.0);
/// // slippage = 0.01 + (5% adjustment) = ~0.06
/// ```
pub fn calculate_slippage(
    expected_price: Decimal,
    market_price: Decimal,
    volatility: f64,
) -> Decimal {
    // Grok's formula: adjust for volatility percentage
    let adjustment = Decimal::from_f64(volatility * 0.01)
        .unwrap_or(Decimal::ZERO);

    // Calculate price difference
    let price_diff = if expected_price > market_price {
        expected_price - market_price
    } else {
        market_price - expected_price
    };

    // Return total slippage: absolute difference + volatility adjustment
    price_diff + adjustment
}

/// Calculate maximum acceptable slippage for a trade
///
/// # Arguments
/// * `expected_price` - Expected execution price
/// * `volatility` - Token volatility percentage
/// * `max_slippage_pct` - Maximum acceptable slippage (default: 2.0%)
///
/// # Returns
/// * Maximum price deviation allowed before rejecting trade
pub fn calculate_max_slippage(
    expected_price: Decimal,
    volatility: f64,
    max_slippage_pct: f64,
) -> Decimal {
    // Base slippage from percentage
    let base_slippage = expected_price * Decimal::from_f64(max_slippage_pct / 100.0)
        .unwrap_or(Decimal::ZERO);

    // Add volatility adjustment
    let volatility_adjustment = expected_price * Decimal::from_f64(volatility / 100.0)
        .unwrap_or(Decimal::ZERO);

    base_slippage + volatility_adjustment
}

/// Validate if trade execution price is within acceptable slippage
///
/// # Arguments
/// * `expected_price` - Price we expected to execute at
/// * `actual_price` - Actual execution price
/// * `volatility` - Token volatility percentage
/// * `max_slippage_pct` - Maximum acceptable slippage percentage
///
/// # Returns
/// * `true` if slippage is acceptable, `false` if trade should be rejected
///
/// # Example
/// ```
/// use rust_decimal::Decimal;
/// let expected = Decimal::from_f64(1.0).unwrap();
/// let actual = Decimal::from_f64(1.015).unwrap();  // 1.5% slip
/// let is_ok = is_slippage_acceptable(expected, actual, 5.0, 2.0);
/// // is_ok = true (within 2% + 5% volatility = 7% total allowed)
/// ```
pub fn is_slippage_acceptable(
    expected_price: Decimal,
    actual_price: Decimal,
    volatility: f64,
    max_slippage_pct: f64,
) -> bool {
    let max_allowed = calculate_max_slippage(expected_price, volatility, max_slippage_pct);

    let actual_slippage = if expected_price > actual_price {
        expected_price - actual_price
    } else {
        actual_price - expected_price
    };

    actual_slippage <= max_allowed
}

/// Calculate slippage percentage for logging/monitoring
///
/// # Returns
/// * Slippage as percentage (e.g., 1.5 for 1.5% slippage)
pub fn calculate_slippage_percentage(
    expected_price: Decimal,
    actual_price: Decimal,
) -> f64 {
    let price_diff = if expected_price > actual_price {
        expected_price - actual_price
    } else {
        actual_price - expected_price
    };

    if expected_price == Decimal::ZERO {
        return 0.0;
    }

    let slippage_pct = (price_diff / expected_price) * Decimal::from(100);
    slippage_pct.to_f64().unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_calculate_slippage() {
        let expected = dec!(1.0);
        let market = dec!(1.01);
        let volatility = 5.0;

        let slippage = calculate_slippage(expected, market, volatility);

        // Should be 0.01 (price diff) + 0.05 (5% volatility) = 0.06
        assert_eq!(slippage, dec!(0.06));
    }

    #[test]
    fn test_is_slippage_acceptable() {
        let expected = dec!(1.0);
        let actual_good = dec!(1.01);   // 1% slippage
        let actual_bad = dec!(1.10);    // 10% slippage

        // With 5% volatility and 2% max slippage = 7% total allowed
        assert!(is_slippage_acceptable(expected, actual_good, 5.0, 2.0));
        assert!(!is_slippage_acceptable(expected, actual_bad, 5.0, 2.0));
    }

    #[test]
    fn test_slippage_percentage() {
        let expected = dec!(1.0);
        let actual = dec!(1.015);

        let pct = calculate_slippage_percentage(expected, actual);

        // Should be 1.5%
        assert!((pct - 1.5).abs() < 0.001);
    }
}
