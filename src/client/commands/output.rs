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
pub(crate) fn print_json<T: Serialize>(value: &T, type_name: &str) {
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
pub(crate) fn print_json_wrapped<T: Serialize>(items: &[T], type_name: &str) {
    let output = serde_json::json!({ "items": items });
    print_json(&output, type_name);
}

/// Conditionally print as JSON or return for table formatting.
///
/// This is a helper for the common pattern where we check format and either
/// print JSON or continue with table formatting.
///
/// The `csv` format is only meaningful for tabular list commands; callers of
/// this helper render a single record, which has no clean CSV representation.
/// Requesting `csv` here prints a clear error and exits with code 1.
///
/// # Returns
/// `true` if JSON was printed, `false` if caller should handle table format
pub(crate) fn print_if_json<T: Serialize>(format: &str, value: &T, type_name: &str) -> bool {
    match format {
        "json" => {
            print_json(value, type_name);
            true
        }
        "csv" => {
            eprintln!(
                "Error: csv format is only supported for list commands; '{}' returns a single record. Use -f json or -f table.",
                type_name
            );
            std::process::exit(1);
        }
        _ => false,
    }
}

/// Conditionally print wrapped collection as JSON or return for table formatting.
///
/// # Returns
/// `true` if JSON was printed, `false` if caller should handle table format
pub(crate) fn print_wrapped_if_json<T: Serialize>(
    format: &str,
    items: &[T],
    type_name: &str,
) -> bool {
    if format == "json" {
        print_json_wrapped(items, type_name);
        true
    } else {
        false
    }
}
