//! Memory utility functions for parsing memory size strings.

/// Convert memory string to bytes.
///
/// Supports formats like "1024", "1k", "2M", "3g", "4T" (case insensitive).
/// Units use binary prefixes (1024-based):
/// - k/K = KiB (1024 bytes)
/// - m/M = MiB (1024² bytes)
/// - g/G = GiB (1024³ bytes)
/// - t/T = TiB (1024⁴ bytes)
///
/// # Examples
///
/// ```
/// use torc::memory_utils::memory_string_to_bytes;
///
/// assert_eq!(memory_string_to_bytes("1024").unwrap(), 1024);
/// assert_eq!(memory_string_to_bytes("1k").unwrap(), 1024);
/// assert_eq!(memory_string_to_bytes("1K").unwrap(), 1024);
/// assert_eq!(memory_string_to_bytes("2M").unwrap(), 2 * 1024 * 1024);
/// assert_eq!(memory_string_to_bytes("1g").unwrap(), 1024 * 1024 * 1024);
/// ```
///
/// # Errors
///
/// Returns an error if:
/// - The string is empty
/// - The unit suffix is invalid
/// - The number part cannot be parsed
/// - The number is negative
/// - The result would overflow i64
pub fn memory_string_to_bytes(memory_str: &str) -> Result<i64, String> {
    let memory_str = memory_str.trim();

    if memory_str.is_empty() {
        return Err("Memory string cannot be empty".to_string());
    }

    // Check if the last character is a unit
    let (number_part, multiplier) = if let Some(last_char) = memory_str.chars().last() {
        if last_char.is_alphabetic() {
            let number_part = &memory_str[..memory_str.len() - 1];
            let multiplier = match last_char.to_ascii_lowercase() {
                'k' => 1024_i64,
                'm' => 1024_i64.pow(2),
                'g' => 1024_i64.pow(3),
                't' => 1024_i64.pow(4),
                _ => return Err(format!("Invalid memory unit: {}", last_char)),
            };
            (number_part, multiplier)
        } else {
            (memory_str, 1_i64)
        }
    } else {
        return Err("Memory string cannot be empty".to_string());
    };

    // Parse the number part
    let number: i64 = number_part
        .parse()
        .map_err(|_| format!("Invalid number in memory string: {}", number_part))?;

    if number < 0 {
        return Err("Memory size cannot be negative".to_string());
    }

    // Calculate total bytes, checking for overflow
    number
        .checked_mul(multiplier)
        .ok_or_else(|| "Memory size too large, would cause overflow".to_string())
}

/// Convert a memory string to megabytes (as u64).
///
/// Supports the same formats as [`memory_string_to_bytes`].
/// Returns `None` if the string is invalid or empty.
///
/// # Examples
///
/// ```
/// use torc::memory_utils::memory_string_to_mb;
///
/// assert_eq!(memory_string_to_mb("1g"), Some(1024));
/// assert_eq!(memory_string_to_mb("512m"), Some(512));
/// assert_eq!(memory_string_to_mb("2G"), Some(2048));
/// assert_eq!(memory_string_to_mb("0"), Some(0));
/// assert_eq!(memory_string_to_mb("500k"), Some(1));
/// assert_eq!(memory_string_to_mb("bad"), None);
/// ```
pub fn memory_string_to_mb(memory_str: &str) -> Option<u64> {
    const MB: i64 = 1024 * 1024;
    memory_string_to_bytes(memory_str).ok().map(|bytes| {
        if bytes == 0 {
            0
        } else {
            (bytes / MB).max(1) as u64
        }
    })
}

/// Convert memory string to gigabytes (as f64).
///
/// This is a convenience function that converts the result of [`memory_string_to_bytes`]
/// to gigabytes.
///
/// # Panics
///
/// Panics if the memory string is invalid.
pub fn memory_string_to_gb(memory_str: &str) -> f64 {
    const GB: i64 = 1024 * 1024 * 1024;
    match memory_string_to_bytes(memory_str) {
        Ok(bytes) => bytes as f64 / GB as f64,
        Err(e) => {
            panic!("Error converting memory string to bytes: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_string_to_bytes_plain() {
        assert_eq!(memory_string_to_bytes("1024").unwrap(), 1024);
        assert_eq!(memory_string_to_bytes("0").unwrap(), 0);
    }

    #[test]
    fn test_memory_string_to_bytes_kibibytes() {
        assert_eq!(memory_string_to_bytes("1k").unwrap(), 1024);
        assert_eq!(memory_string_to_bytes("1K").unwrap(), 1024);
        assert_eq!(memory_string_to_bytes("2k").unwrap(), 2048);
    }

    #[test]
    fn test_memory_string_to_bytes_mebibytes() {
        assert_eq!(memory_string_to_bytes("1m").unwrap(), 1024 * 1024);
        assert_eq!(memory_string_to_bytes("1M").unwrap(), 1024 * 1024);
        assert_eq!(memory_string_to_bytes("2M").unwrap(), 2 * 1024 * 1024);
    }

    #[test]
    fn test_memory_string_to_bytes_gibibytes() {
        assert_eq!(memory_string_to_bytes("1g").unwrap(), 1024 * 1024 * 1024);
        assert_eq!(memory_string_to_bytes("1G").unwrap(), 1024 * 1024 * 1024);
    }

    #[test]
    fn test_memory_string_to_bytes_tebibytes() {
        assert_eq!(
            memory_string_to_bytes("1t").unwrap(),
            1024_i64 * 1024 * 1024 * 1024
        );
        assert_eq!(
            memory_string_to_bytes("1T").unwrap(),
            1024_i64 * 1024 * 1024 * 1024
        );
    }

    #[test]
    fn test_memory_string_to_bytes_with_whitespace() {
        assert_eq!(memory_string_to_bytes("  1k  ").unwrap(), 1024);
    }

    #[test]
    fn test_memory_string_to_bytes_empty() {
        assert!(memory_string_to_bytes("").is_err());
        assert!(memory_string_to_bytes("   ").is_err());
    }

    #[test]
    fn test_memory_string_to_bytes_invalid_unit() {
        assert!(memory_string_to_bytes("1x").is_err());
    }

    #[test]
    fn test_memory_string_to_bytes_negative() {
        assert!(memory_string_to_bytes("-1k").is_err());
    }

    #[test]
    fn test_memory_string_to_gb() {
        assert!((memory_string_to_gb("1g") - 1.0).abs() < 0.001);
        assert!((memory_string_to_gb("2G") - 2.0).abs() < 0.001);
        assert!((memory_string_to_gb("512M") - 0.5).abs() < 0.001);
    }
}
