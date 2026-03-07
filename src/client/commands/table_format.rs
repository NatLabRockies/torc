use tabled::settings::location::ByColumnName;
use tabled::settings::{Remove, Style};
use tabled::{Table, Tabled};

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
}
