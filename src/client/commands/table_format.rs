use tabled::settings::location::ByColumnName;
use tabled::settings::{Remove, Style};
use tabled::{Table, Tabled};

/// Render a slice of `Tabled` rows as RFC 4180 CSV to stdout.
///
/// The CSV header row reuses the same column names as the table view (the
/// `#[tabled(rename = "...")]` headers), and each data row reuses the same
/// stringified cell values. A header row is always emitted, even for an empty
/// slice, so downstream tooling always sees the column names.
///
/// Errors writing CSV (e.g. a broken pipe) are reported to stderr and exit 1.
pub fn display_csv<T: Tabled>(items: &[T]) {
    print!("{}", build_csv(items));
}

/// Render a slice of `Tabled` rows as CSV, excluding the named columns
/// (case-insensitive match against the headers). Unknown column names are
/// reported as warnings on stderr, matching `display_table_excluding`.
pub fn display_csv_excluding<T: Tabled>(items: &[T], exclude_columns: &[String]) {
    for col in exclude_columns {
        if !T::headers()
            .iter()
            .any(|h| h.to_lowercase() == col.to_lowercase())
        {
            let headers: Vec<String> = T::headers().into_iter().map(|h| h.to_string()).collect();
            eprintln!(
                "Warning: column '{}' not found. Available columns: {}",
                col,
                headers.join(", ")
            );
        }
    }
    print!("{}", build_csv_excluding(items, exclude_columns));
}

/// Build the CSV representation of `items` as a `String` (header + data rows).
fn build_csv<T: Tabled>(items: &[T]) -> String {
    let headers: Vec<String> = T::headers().into_iter().map(|h| h.to_string()).collect();
    let rows: Vec<Vec<String>> = items
        .iter()
        .map(|item| item.fields().into_iter().map(|f| f.to_string()).collect())
        .collect();
    csv_string(&headers, &rows)
}

/// Build the CSV representation of `items`, dropping the excluded columns.
fn build_csv_excluding<T: Tabled>(items: &[T], exclude_columns: &[String]) -> String {
    let headers: Vec<String> = T::headers().into_iter().map(|h| h.to_string()).collect();
    let exclude_lower: Vec<String> = exclude_columns.iter().map(|c| c.to_lowercase()).collect();

    // Indices of columns to keep (those whose header is not excluded).
    let keep: Vec<usize> = headers
        .iter()
        .enumerate()
        .filter(|(_, h)| !exclude_lower.contains(&h.to_lowercase()))
        .map(|(i, _)| i)
        .collect();

    let kept_headers: Vec<String> = keep.iter().map(|&i| headers[i].clone()).collect();
    let rows: Vec<Vec<String>> = items
        .iter()
        .map(|item| {
            let fields: Vec<String> = item.fields().into_iter().map(|f| f.to_string()).collect();
            keep.iter().map(|&i| fields[i].clone()).collect()
        })
        .collect();
    csv_string(&kept_headers, &rows)
}

/// Serialize a header row plus data rows into an RFC 4180 CSV string.
fn csv_string(headers: &[String], rows: &[Vec<String>]) -> String {
    let mut wtr = csv::Writer::from_writer(Vec::new());
    let write = |wtr: &mut csv::Writer<Vec<u8>>| -> csv::Result<()> {
        wtr.write_record(headers)?;
        for row in rows {
            wtr.write_record(row)?;
        }
        wtr.flush()?;
        Ok(())
    };
    if let Err(e) = write(&mut wtr) {
        eprintln!("Error writing CSV output: {}", e);
        std::process::exit(1);
    }
    match wtr.into_inner() {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(e) => {
            eprintln!("Error finalizing CSV output: {}", e);
            std::process::exit(1);
        }
    }
}

/// Conditionally render rows as CSV.
///
/// Returns `true` if `format` is `"csv"` (CSV was printed and the caller should
/// skip its human-readable preamble / empty-state messages), otherwise `false`.
pub fn display_csv_if_csv<T: Tabled>(format: &str, items: &[T]) -> bool {
    if format == "csv" {
        display_csv(items);
        true
    } else {
        false
    }
}

/// Display a collection of items as a formatted table
pub fn display_table<T: Tabled>(items: &[T]) {
    if items.is_empty() {
        return;
    }

    let mut table = Table::new(items);
    table.with(Style::rounded());
    println!("{}", table);
}

/// Display a collection of items as a formatted table with a custom title
pub fn display_table_with_title<T: Tabled>(items: &[T], title: &str) {
    if items.is_empty() {
        println!("{}", title);
        return;
    }

    println!("{}", title);
    let mut table = Table::new(items);
    table.with(Style::rounded());
    println!("{}", table);
}

/// Display a collection of items as a formatted table with a total count
pub fn display_table_with_count<T: Tabled>(items: &[T], item_type: &str) {
    if items.is_empty() {
        return;
    }

    let mut table = Table::new(items);
    table.with(Style::rounded());
    println!("{}", table);
    println!("\nTotal: {} {}", items.len(), item_type);
}

/// Build a table string with specified columns excluded (case-insensitive match).
/// Returns the table string and a list of any column names that were not found.
pub fn build_table_excluding<T: Tabled>(
    items: &[T],
    exclude_columns: &[String],
) -> (String, Vec<String>) {
    let mut table = Table::new(items);
    table.with(Style::rounded());

    let headers: Vec<String> = T::headers().into_iter().map(|h| h.to_string()).collect();
    let mut not_found = Vec::new();

    for col in exclude_columns {
        let col_lower = col.to_lowercase();
        if let Some(header) = headers.iter().find(|h| h.to_lowercase() == col_lower) {
            table.with(Remove::column(ByColumnName::new(header.clone())));
        } else {
            not_found.push(col.clone());
        }
    }

    (table.to_string(), not_found)
}

/// Display a table with specified columns excluded (case-insensitive match).
pub fn display_table_excluding<T: Tabled>(
    items: &[T],
    exclude_columns: &[String],
    item_type: &str,
) {
    if items.is_empty() {
        return;
    }

    let (table_str, not_found) = build_table_excluding(items, exclude_columns);

    let headers: Vec<String> = T::headers().into_iter().map(|h| h.to_string()).collect();
    for col in &not_found {
        eprintln!(
            "Warning: column '{}' not found. Available columns: {}",
            col,
            headers.join(", ")
        );
    }

    println!("{}", table_str);
    println!("\nTotal: {} {}", items.len(), item_type);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Tabled)]
    struct TestRow {
        #[tabled(rename = "Name")]
        name: String,
        #[tabled(rename = "Command")]
        command: String,
        #[tabled(rename = "Status")]
        status: String,
    }

    fn sample_rows() -> Vec<TestRow> {
        vec![
            TestRow {
                name: "job1".into(),
                command: "echo hello".into(),
                status: "completed".into(),
            },
            TestRow {
                name: "job2".into(),
                command: "sleep 10".into(),
                status: "running".into(),
            },
        ]
    }

    #[test]
    fn test_exclude_single_column() {
        let rows = sample_rows();
        let (table, not_found) = build_table_excluding(&rows, &["command".to_string()]);
        assert!(not_found.is_empty());
        assert!(table.contains("Name"));
        assert!(table.contains("Status"));
        assert!(!table.contains("Command"));
        assert!(!table.contains("echo hello"));
    }

    #[test]
    fn test_exclude_multiple_columns() {
        let rows = sample_rows();
        let (table, not_found) =
            build_table_excluding(&rows, &["command".to_string(), "status".to_string()]);
        assert!(not_found.is_empty());
        assert!(table.contains("Name"));
        assert!(!table.contains("Command"));
        assert!(!table.contains("Status"));
    }

    #[test]
    fn test_exclude_case_insensitive() {
        let rows = sample_rows();
        let (table, not_found) = build_table_excluding(&rows, &["COMMAND".to_string()]);
        assert!(not_found.is_empty());
        assert!(!table.contains("Command"));
    }

    #[test]
    fn test_exclude_unknown_column() {
        let rows = sample_rows();
        let (table, not_found) = build_table_excluding(&rows, &["nonexistent".to_string()]);
        assert_eq!(not_found, vec!["nonexistent"]);
        // All columns still present
        assert!(table.contains("Name"));
        assert!(table.contains("Command"));
        assert!(table.contains("Status"));
    }

    #[test]
    fn test_exclude_no_columns() {
        let rows = sample_rows();
        let (table, not_found) = build_table_excluding(&rows, &[]);
        assert!(not_found.is_empty());
        assert!(table.contains("Name"));
        assert!(table.contains("Command"));
        assert!(table.contains("Status"));
    }

    #[test]
    fn test_csv_uses_renamed_headers() {
        let csv = build_csv(&sample_rows());
        let first_line = csv.lines().next().unwrap();
        assert_eq!(first_line, "Name,Command,Status");
    }

    #[test]
    fn test_csv_rows_match_table_cells() {
        let csv = build_csv(&sample_rows());
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 3); // header + 2 rows
        assert_eq!(lines[1], "job1,echo hello,completed");
        assert_eq!(lines[2], "job2,sleep 10,running");
    }

    #[test]
    fn test_csv_header_emitted_when_empty() {
        let rows: Vec<TestRow> = Vec::new();
        let csv = build_csv(&rows);
        assert_eq!(csv.trim_end(), "Name,Command,Status");
    }

    #[test]
    fn test_csv_quotes_fields_with_special_chars() {
        let rows = vec![TestRow {
            name: "weird".into(),
            command: "echo \"hi\", then go".into(),
            status: "done".into(),
        }];
        let csv = build_csv(&rows);
        let data_line = csv.lines().nth(1).unwrap();
        // A field containing a comma and quotes must be quoted, with inner
        // quotes doubled, per RFC 4180.
        assert_eq!(data_line, r#"weird,"echo ""hi"", then go",done"#);
    }

    #[test]
    fn test_csv_excluding_drops_named_column() {
        let csv = build_csv_excluding(&sample_rows(), &["command".to_string()]);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "Name,Status");
        assert_eq!(lines[1], "job1,completed");
        assert_eq!(lines[2], "job2,running");
    }
}
