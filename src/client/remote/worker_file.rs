//! Worker file parsing for remote execution.

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use super::types::WorkerEntry;

/// Parse a worker file into a list of WorkerEntry.
///
/// The file format is:
/// - Lines starting with `#` are comments
/// - Empty lines are ignored
/// - Each line is: `[user@]hostname[:port]`
///
/// # Examples
///
/// ```text
/// # Comment line
/// worker1.example.com
/// user@192.168.1.10
/// admin@server.local:2222
/// ```
pub(crate) fn parse_worker_file(path: &Path) -> Result<Vec<WorkerEntry>, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read worker file '{}': {}", path.display(), e))?;

    parse_worker_content(&content, path.to_string_lossy().as_ref())
}

/// Parse worker file content.
///
/// This function is public for use in tests.
pub fn parse_worker_content(content: &str, source: &str) -> Result<Vec<WorkerEntry>, String> {
    let mut workers = Vec::new();
    let mut seen_hosts = HashSet::new();

    for (line_num, line) in content.lines().enumerate() {
        let line_num = line_num + 1; // 1-indexed for error messages
        let trimmed = line.trim();

        // Skip empty lines and comments
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let entry = parse_worker_line(trimmed, line_num, source)?;

        // Check for duplicates (by host, ignoring user/port differences)
        if seen_hosts.contains(&entry.host) {
            return Err(format!(
                "{}:{}: Duplicate host '{}' (each host should only appear once)",
                source, line_num, entry.host
            ));
        }
        seen_hosts.insert(entry.host.clone());

        workers.push(entry);
    }

    if workers.is_empty() {
        return Err(format!(
            "Worker file '{}' contains no valid entries",
            source
        ));
    }

    Ok(workers)
}

/// Parse a single line from the worker file.
fn parse_worker_line(line: &str, line_num: usize, source: &str) -> Result<WorkerEntry, String> {
    let original = line.to_string();

    // Format: [user@]hostname[:port]
    // First, split off the user if present
    let (user, host_port) = if let Some(at_pos) = line.find('@') {
        let user = &line[..at_pos];
        let rest = &line[at_pos + 1..];

        if user.is_empty() {
            return Err(format!(
                "{}:{}: Empty username before '@' in '{}'",
                source, line_num, line
            ));
        }

        (Some(user.to_string()), rest)
    } else {
        (None, line)
    };

    // Now split off the port if present
    // Handle IPv6 addresses: [::1]:22 or [2001:db8::1]:22
    let (host, port) = if host_port.starts_with('[') {
        // IPv6 address in brackets
        if let Some(bracket_end) = host_port.find(']') {
            let ipv6 = &host_port[1..bracket_end];
            let rest = &host_port[bracket_end + 1..];
            if rest.is_empty() {
                (ipv6.to_string(), None)
            } else if let Some(port_str) = rest.strip_prefix(':') {
                let port: u16 = port_str.parse().map_err(|_| {
                    format!(
                        "{}:{}: Invalid port '{}' in '{}'",
                        source, line_num, port_str, line
                    )
                })?;
                (ipv6.to_string(), Some(port))
            } else {
                return Err(format!(
                    "{}:{}: Invalid format after IPv6 address in '{}'",
                    source, line_num, line
                ));
            }
        } else {
            return Err(format!(
                "{}:{}: Unclosed bracket in IPv6 address '{}'",
                source, line_num, line
            ));
        }
    } else {
        // Regular hostname or IPv4
        // Split on the last colon to handle port
        if let Some(colon_pos) = host_port.rfind(':') {
            let host = &host_port[..colon_pos];
            let port_str = &host_port[colon_pos + 1..];

            // Make sure port looks like a number (to avoid treating IPv6 as host:port)
            if port_str.chars().all(|c| c.is_ascii_digit()) && !port_str.is_empty() {
                let port: u16 = port_str.parse().map_err(|_| {
                    format!(
                        "{}:{}: Invalid port '{}' in '{}'",
                        source, line_num, port_str, line
                    )
                })?;
                (host.to_string(), Some(port))
            } else {
                // Not a port, treat the whole thing as the host
                (host_port.to_string(), None)
            }
        } else {
            (host_port.to_string(), None)
        }
    };

    if host.is_empty() {
        return Err(format!(
            "{}:{}: Empty hostname in '{}'",
            source, line_num, line
        ));
    }

    Ok(WorkerEntry {
        original,
        user,
        host,
        port,
    })
}
