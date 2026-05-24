//! Output formatting utilities for CLI commands.
//!
//! This module provides helper functions for consistent JSON output formatting
//! across all command handlers, reducing code duplication and ensuring consistent
//! error handling.

use serde::Serialize;

/// Print a single object as pretty-printed JSON.
///
/// This function handles serialization errors consistently by printing to stderr
/// and exiting with code 1.
///
/// # Arguments
/// * `value` - Any serializable value to print
/// * `type_name` - Human-readable name of the type for error messages
///
/// # Example
/// ```ignore
/// print_json(&job, "job");
/// ```
pub fn print_json<T: Serialize>(value: &T, type_name: &str) {
    match serde_json::to_string_pretty(value) {
        Ok(json) => println!("{}", json),
        Err(e) => {
            eprintln!("Error serializing {} to JSON: {}", type_name, e);
            std::process::exit(1);
        }
    }
}

/// Print a collection wrapped in a JSON object under the `items` field.
///
/// Output format: `{"items": [...]}`
///
/// All list commands use the same `items` key for consistency with the REST API
/// (whose paginated responses also wrap records in `items`) and so that generic
/// tooling (e.g. `jq '.items[]'`) works uniformly across resource types.
///
/// # Arguments
/// * `items` - A slice of serializable items
/// * `type_name` - Human-readable name for error messages
///
/// # Example
/// ```ignore
/// print_json_wrapped(&jobs, "jobs");
/// // Outputs: {"items": [...]}
/// ```
pub fn print_json_wrapped<T: Serialize>(items: &[T], type_name: &str) {
    let output = serde_json::json!({ "items": items });
    print_json(&output, type_name);
}

/// Print a success response with workflow ID.
///
/// Output format: `{"workflow_id": N, "status": "success", "message": "..."}`
///
/// # Arguments
/// * `workflow_id` - The workflow ID to include
/// * `message` - Success message to display
pub fn print_success_response(workflow_id: i64, message: &str) {
    let output = serde_json::json!({
        "workflow_id": workflow_id,
        "status": "success",
        "message": message
    });
    // Success responses are simple and should never fail to serialize
    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}

/// Print a success response with custom status and optional extra fields.
///
/// # Arguments
/// * `workflow_id` - The workflow ID to include
/// * `status` - Status string (e.g., "success", "partial_success")
/// * `message` - Message to display
/// * `extra` - Optional additional JSON value to merge into response
pub fn print_status_response(
    workflow_id: i64,
    status: &str,
    message: &str,
    extra: Option<serde_json::Value>,
) {
    let mut output = serde_json::json!({
        "workflow_id": workflow_id,
        "status": status,
        "message": message
    });
    if let Some(extra_fields) = extra
        && let Some(obj) = extra_fields.as_object()
    {
        for (key, value) in obj {
            output[key] = value.clone();
        }
    }
    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}

/// Print an error response to stderr.
///
/// Output format: `{"status": "error", "message": "...", ...}`
///
/// Note: This function does NOT exit. Call `std::process::exit(1)` after if needed.
///
/// # Arguments
/// * `message` - Error message to display
/// * `details` - Optional additional JSON value with error details
pub fn print_error_json(message: &str, details: Option<serde_json::Value>) {
    let mut output = serde_json::json!({
        "status": "error",
        "message": message
    });
    if let Some(d) = details {
        output["details"] = d;
    }
    eprintln!("{}", serde_json::to_string_pretty(&output).unwrap());
}

/// Print an error response to stderr and exit with code 1.
///
/// # Arguments
/// * `message` - Error message to display
/// * `details` - Optional additional JSON value with error details
pub fn print_error_json_and_exit(message: &str, details: Option<serde_json::Value>) -> ! {
    print_error_json(message, details);
    std::process::exit(1);
}

/// Conditionally print as JSON or return for table formatting.
///
/// This is a helper for the common pattern where we check format and either
/// print JSON or continue with table formatting.
///
/// # Returns
/// `true` if JSON was printed, `false` if caller should handle table format
pub fn print_if_json<T: Serialize>(format: &str, value: &T, type_name: &str) -> bool {
    if format == "json" {
        print_json(value, type_name);
        true
    } else {
        false
    }
}

/// Conditionally print wrapped collection as JSON or return for table formatting.
///
/// # Returns
/// `true` if JSON was printed, `false` if caller should handle table format
pub fn print_wrapped_if_json<T: Serialize>(format: &str, items: &[T], type_name: &str) -> bool {
    if format == "json" {
        print_json_wrapped(items, type_name);
        true
    } else {
        false
    }
}
