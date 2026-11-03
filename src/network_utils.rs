//! Network utility functions for Torc.
//!
//! This module provides shared network-related utilities such as finding available ports.

use std::io::ErrorKind;
use std::net::TcpListener;

/// Maximum number of ports to try when searching for an available port.
const MAX_PORT_ATTEMPTS: u16 = 100;

/// Try to bind to a port, incrementing if the port is in use.
///
/// This function attempts to bind to `start_port` and, if that port is already in use,
/// tries successive ports (start_port + 1, start_port + 2, etc.) until an available
/// port is found or the maximum number of attempts is reached.
///
/// # Arguments
///
/// * `host` - The host address to bind to (e.g., "127.0.0.1" or "0.0.0.0")
/// * `start_port` - The preferred port to start with
///
/// # Returns
///
/// A tuple containing the bound `std::net::TcpListener` and the actual port number.
/// The caller can convert to a tokio listener if needed:
///
/// ```ignore
/// let (std_listener, port) = find_available_port("127.0.0.1", 8080)?;
/// std_listener.set_nonblocking(true)?;
/// let tokio_listener = tokio::net::TcpListener::from_std(std_listener)?;
/// ```
///
/// # Errors
///
/// Returns an error if:
/// - No available port is found after `MAX_PORT_ATTEMPTS` attempts
/// - A non-"address in use" error occurs (e.g., permission denied)
/// - The port number reaches `u16::MAX`
///
/// # Example
///
/// ```no_run
/// use torc::network_utils::find_available_port;
///
/// fn main() -> anyhow::Result<()> {
///     let (listener, port) = find_available_port("127.0.0.1", 8080)?;
///     println!("Bound to port {}", port);
///     Ok(())
/// }
/// ```
pub fn find_available_port(host: &str, start_port: u16) -> anyhow::Result<(TcpListener, u16)> {
    let mut port = start_port;
    let mut attempts = 0;

    loop {
        let addr = format!("{}:{}", host, port);
        match TcpListener::bind(&addr) {
            Ok(listener) => return Ok((listener, port)),
            Err(e) => {
                attempts += 1;
                if attempts >= MAX_PORT_ATTEMPTS {
                    return Err(anyhow::anyhow!(
                        "Failed to find available port after {} attempts (tried ports {}-{}): {}",
                        MAX_PORT_ATTEMPTS,
                        start_port,
                        port,
                        e
                    ));
                }
                // Check if it's an "address in use" error
                if e.kind() == ErrorKind::AddrInUse {
                    // Avoid exceeding the maximum port number
                    if port == u16::MAX {
                        return Err(anyhow::anyhow!(
                            "Port number reached maximum value while searching for available port"
                        ));
                    }
                    let next_port = port + 1;
                    log::info!("Port {} is in use, trying port {}", port, next_port);
                    port = next_port;
                } else {
                    // Different error (e.g., permission denied), don't retry
                    return Err(anyhow::anyhow!("Failed to bind to {}: {}", addr, e));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_available_port_succeeds() {
        // Should find an available port starting from a high port number
        let result = find_available_port("127.0.0.1", 19000);
        assert!(result.is_ok());
        let (listener, port) = result.unwrap();
        assert!(port >= 19000);
        drop(listener);
    }

    #[test]
    fn test_find_available_port_increments_on_collision() {
        // Bind to a port first
        let first_listener = TcpListener::bind("127.0.0.1:19100").unwrap();
        let first_port = first_listener.local_addr().unwrap().port();

        // Try to find a port starting from the same port - should get the next one
        let result = find_available_port("127.0.0.1", first_port);
        assert!(result.is_ok());
        let (_second_listener, second_port) = result.unwrap();
        assert!(second_port > first_port);

        drop(first_listener);
    }

    #[test]
    fn test_find_available_port_max_port_error() {
        // Starting from u16::MAX with a port already in use should fail
        // since there's no port after 65535 to try.

        // Try to bind to port 65535 to block it
        if let Ok(_blocker) = TcpListener::bind("127.0.0.1:65535") {
            // Port 65535 is now blocked, try to find an available port starting there
            let result = find_available_port("127.0.0.1", 65535);
            assert!(result.is_err());
            let err_msg = result.unwrap_err().to_string();
            assert!(
                err_msg.contains("maximum value"),
                "Expected 'maximum value' error, got: {}",
                err_msg
            );
        }
        // If we can't bind to 65535 (e.g., permission denied), skip the test
    }

    #[test]
    fn test_find_available_port_non_addr_in_use_error() {
        // Using an invalid host should trigger a non-AddrInUse error
        // and should fail immediately without retrying
        let result = find_available_port("999.999.999.999", 8080);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        // Should get "Failed to bind" error, not "Failed to find available port after N attempts"
        assert!(
            err_msg.contains("Failed to bind"),
            "Expected 'Failed to bind' error, got: {}",
            err_msg
        );
        assert!(
            !err_msg.contains("attempts"),
            "Should not have retried on non-AddrInUse error, got: {}",
            err_msg
        );
    }
}
