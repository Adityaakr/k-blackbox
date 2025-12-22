use rust_decimal::Decimal;
use std::str::FromStr;

/// Format a Decimal to a fixed number of decimal places, then apply Kraken's
/// checksum formatting rules: remove '.', trim leading zeros.
/// 
/// Steps:
/// 1. Format to exactly `scale` decimal places (pad with zeros if needed)
/// 2. Remove the decimal point
/// 3. Remove leading zeros (but keep at least "0" if empty)
pub fn format_fixed(dec: &Decimal, scale: u32) -> String {
    // Use Decimal's formatting to get exactly `scale` decimal places
    let formatted = if scale > 0 {
        // Round to scale decimal places
        let rounded = dec.round_dp(scale);
        let mut s = rounded.to_string();
        
        // Ensure we have exactly `scale` decimal places
        if s.contains('.') {
            let parts: Vec<&str> = s.split('.').collect();
            let integer_part = parts[0];
            let decimal_part = parts.get(1).unwrap_or(&"");
            
            let mut decimal_padded = decimal_part.to_string();
            if decimal_padded.len() < scale as usize {
                decimal_padded.push_str(&"0".repeat(scale as usize - decimal_padded.len()));
            } else if decimal_padded.len() > scale as usize {
                decimal_padded.truncate(scale as usize);
            }
            
            s = format!("{}.{}", integer_part, decimal_padded);
        } else {
            // No decimal point, add it with zeros
            s = format!("{}.{}", s, "0".repeat(scale as usize));
        }
        
        s
    } else {
        // No decimal places needed
        dec.to_string()
    };
    
    // Remove decimal point
    let mut result = formatted.replace('.', "");
    
    // Remove leading zeros, but keep at least one digit
    result = result.trim_start_matches('0').to_string();
    if result.is_empty() {
        result = "0".to_string();
    }
    
    result
}

/// Parse a string as Decimal, preserving full precision
/// Handles both regular decimal notation and scientific notation (e.g., "1e-8")
pub fn parse_decimal(s: &str) -> anyhow::Result<Decimal> {
    // Try parsing directly first
    if let Ok(dec) = Decimal::from_str(s) {
        return Ok(dec);
    }
    
    // If that fails, try parsing as f64 first (handles scientific notation)
    // then convert to Decimal
    if let Ok(f) = s.parse::<f64>() {
        Decimal::from_str_exact(&format!("{}", f))
            .or_else(|_| Decimal::try_from(f))
            .map_err(|e| anyhow::anyhow!("Failed to parse decimal '{}': {}", s, e))
    } else {
        Err(anyhow::anyhow!("Failed to parse decimal '{}': Invalid format", s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_format_fixed() {
        // Test with 2 decimal places
        assert_eq!(format_fixed(&dec!(123.45), 2), "12345");
        assert_eq!(format_fixed(&dec!(0.01), 2), "1");
        assert_eq!(format_fixed(&dec!(0.10), 2), "10");
        assert_eq!(format_fixed(&dec!(100.00), 2), "10000");
        assert_eq!(format_fixed(&dec!(0.00), 2), "0");
        
        // Test with 8 decimal places (common for crypto)
        assert_eq!(format_fixed(&dec!(50000.12345678), 8), "5000012345678");
        assert_eq!(format_fixed(&dec!(0.00000001), 8), "1");
    }
}

