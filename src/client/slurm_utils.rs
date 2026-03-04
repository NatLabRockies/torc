//! Slurm-specific parsing utilities.
//!
//! Provides helpers for parsing values returned by Slurm accounting commands
//! (`sacct`, `sstat`), such as memory strings (`"512K"`, `"2G"`) and CPU time
//! strings (`[D-]HH:MM:SS`).

/// Parse a Slurm memory string (e.g. "512K", "1.50M", "2G") into bytes.
/// Returns `None` for empty or unparseable values; `Some(0)` for "0".
pub(crate) fn parse_slurm_memory(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if s == "0" {
        return Some(0);
    }
    let (num_str, multiplier) = if let Some(rest) = s.strip_suffix('K') {
        (rest, 1_024i64)
    } else if let Some(rest) = s.strip_suffix('M') {
        (rest, 1_024 * 1_024)
    } else if let Some(rest) = s.strip_suffix('G') {
        (rest, 1_024 * 1_024 * 1_024)
    } else if let Some(rest) = s.strip_suffix('T') {
        (rest, 1_024 * 1_024 * 1_024 * 1_024)
    } else {
        (s, 1)
    };
    let n: f64 = num_str.parse().ok()?;
    Some((n * multiplier as f64) as i64)
}

/// Parse a Slurm CPU time string (`[D-]HH:MM:SS`) into seconds.
/// Returns `None` for empty or unparseable values.
pub(crate) fn parse_slurm_cpu_time(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let (days, rest) = if let Some(dash) = s.find('-') {
        let d: u64 = s[..dash].parse().ok()?;
        (d, &s[dash + 1..])
    } else {
        (0, s)
    };
    let parts: Vec<&str> = rest.split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    let h: u64 = parts[0].parse().ok()?;
    let m: u64 = parts[1].parse().ok()?;
    let sec: f64 = parts[2].parse().ok()?;
    Some((days * 86_400 + h * 3_600 + m * 60) as f64 + sec)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_slurm_memory_units() {
        assert_eq!(parse_slurm_memory("0"), Some(0));
        assert_eq!(parse_slurm_memory("512K"), Some(512 * 1_024));
        assert_eq!(parse_slurm_memory("2M"), Some(2 * 1_024 * 1_024));
        assert_eq!(parse_slurm_memory("1G"), Some(1_024 * 1_024 * 1_024));
        assert_eq!(
            parse_slurm_memory("1T"),
            Some(1_024 * 1_024 * 1_024 * 1_024)
        );
    }

    #[test]
    fn test_parse_slurm_memory_decimal() {
        // sacct can emit fractional values like "1.50M"
        let result = parse_slurm_memory("1.50M").unwrap();
        assert!((result as f64 - 1.5 * 1_024.0 * 1_024.0).abs() < 1.0);
    }

    #[test]
    fn test_parse_slurm_memory_no_suffix() {
        // Raw bytes
        assert_eq!(parse_slurm_memory("1024"), Some(1024));
    }

    #[test]
    fn test_parse_slurm_memory_empty() {
        assert_eq!(parse_slurm_memory(""), None);
        assert_eq!(parse_slurm_memory("  "), None);
    }

    #[test]
    fn test_parse_slurm_cpu_time_hhmmss() {
        assert_eq!(parse_slurm_cpu_time("00:01:30"), Some(90.0));
        assert_eq!(parse_slurm_cpu_time("01:00:00"), Some(3_600.0));
        assert_eq!(parse_slurm_cpu_time("00:00:00"), Some(0.0));
    }

    #[test]
    fn test_parse_slurm_cpu_time_with_days() {
        // Format: D-HH:MM:SS
        assert_eq!(parse_slurm_cpu_time("1-02:30:00"), Some(95_400.0));
        assert_eq!(parse_slurm_cpu_time("0-00:00:01"), Some(1.0));
    }

    #[test]
    fn test_parse_slurm_cpu_time_empty() {
        assert_eq!(parse_slurm_cpu_time(""), None);
        assert_eq!(parse_slurm_cpu_time("  "), None);
    }

    #[test]
    fn test_parse_slurm_cpu_time_fractional_seconds() {
        // Some sacct versions emit sub-second values
        let result = parse_slurm_cpu_time("00:00:01.5").unwrap();
        assert!((result - 1.5).abs() < 0.001);
    }
}
