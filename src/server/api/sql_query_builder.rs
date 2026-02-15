//! SQL query builder utility for pagination and sorting

use log::warn;

/// Utility for building SQL queries with pagination and sorting
pub struct SqlQueryBuilder {
    base_query: String,
    where_clause: Option<String>,
    order_by_clause: Option<String>,
    limit_clause: Option<String>,
    offset_clause: Option<String>,
}

impl SqlQueryBuilder {
    pub fn new(base_query: String) -> Self {
        Self {
            base_query,
            where_clause: None,
            order_by_clause: None,
            limit_clause: None,
            offset_clause: None,
        }
    }

    pub fn with_where(mut self, where_clause: String) -> Self {
        self.where_clause = Some(where_clause);
        self
    }

    pub fn with_pagination_and_sorting(
        mut self,
        offset: i64,
        limit: i64,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        default_sort_column: &str,
        allowed_columns: &[&str],
    ) -> Self {
        let sort_column = sort_by
            .filter(|s| !s.is_empty())
            .filter(|col| {
                // Defense-in-depth: validate sort column against the allowed list.
                // Strip a single table-alias prefix (e.g., "r.id" -> "id") so callers
                // that add table prefixes still pass validation.
                let bare = col
                    .split_once('.')
                    .map(|(_, name)| name)
                    .unwrap_or(col.as_str());
                let valid = allowed_columns.contains(&bare);
                if !valid {
                    warn!(
                        "SqlQueryBuilder: rejected sort column '{}' (not in allowed list), \
                         falling back to default '{}'",
                        col, default_sort_column
                    );
                }
                valid
            })
            .unwrap_or_else(|| default_sort_column.to_string());
        let sort_direction = if reverse_sort.unwrap_or(false) {
            "DESC"
        } else {
            "ASC"
        };
        self.order_by_clause = Some(format!("ORDER BY {} {}", sort_column, sort_direction));

        self.limit_clause = Some(format!("LIMIT {}", limit));

        if offset > 0 {
            self.offset_clause = Some(format!("OFFSET {}", offset));
        }

        self
    }

    pub fn build(self) -> String {
        let mut query = self.base_query;

        if let Some(where_clause) = self.where_clause {
            query.push_str(" WHERE ");
            query.push_str(&where_clause);
        }

        if let Some(order_by) = self.order_by_clause {
            query.push(' ');
            query.push_str(&order_by);
        }

        if let Some(limit) = self.limit_clause {
            query.push(' ');
            query.push_str(&limit);
        }

        if let Some(offset) = self.offset_clause {
            query.push(' ');
            query.push_str(&offset);
        }

        query
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALLOWED: &[&str] = &["id", "name", "status"];

    #[test]
    fn test_valid_sort_column() {
        let q = SqlQueryBuilder::new("SELECT * FROM job".to_string())
            .with_pagination_and_sorting(0, 10, Some("name".to_string()), None, "id", ALLOWED)
            .build();
        assert!(q.contains("ORDER BY name ASC"));
    }

    #[test]
    fn test_invalid_sort_column_falls_back_to_default() {
        let q = SqlQueryBuilder::new("SELECT * FROM job".to_string())
            .with_pagination_and_sorting(0, 10, Some("DROP TABLE".to_string()), None, "id", ALLOWED)
            .build();
        assert!(q.contains("ORDER BY id ASC"));
        assert!(!q.contains("DROP TABLE"));
    }

    #[test]
    fn test_table_alias_prefix_passes_validation() {
        let q = SqlQueryBuilder::new("SELECT * FROM job j".to_string())
            .with_pagination_and_sorting(0, 10, Some("j.name".to_string()), None, "j.id", ALLOWED)
            .build();
        assert!(q.contains("ORDER BY j.name ASC"));
    }

    #[test]
    fn test_none_sort_uses_default() {
        let q = SqlQueryBuilder::new("SELECT * FROM job".to_string())
            .with_pagination_and_sorting(0, 10, None, None, "id", ALLOWED)
            .build();
        assert!(q.contains("ORDER BY id ASC"));
    }

    #[test]
    fn test_empty_sort_uses_default() {
        let q = SqlQueryBuilder::new("SELECT * FROM job".to_string())
            .with_pagination_and_sorting(0, 10, Some(String::new()), None, "id", ALLOWED)
            .build();
        assert!(q.contains("ORDER BY id ASC"));
    }

    #[test]
    fn test_reverse_sort() {
        let q = SqlQueryBuilder::new("SELECT * FROM job".to_string())
            .with_pagination_and_sorting(0, 10, Some("name".to_string()), Some(true), "id", ALLOWED)
            .build();
        assert!(q.contains("ORDER BY name DESC"));
    }

    #[test]
    fn test_offset_included_when_positive() {
        let q = SqlQueryBuilder::new("SELECT * FROM job".to_string())
            .with_pagination_and_sorting(5, 10, None, None, "id", ALLOWED)
            .build();
        assert!(q.contains("OFFSET 5"));
    }

    #[test]
    fn test_offset_omitted_when_zero() {
        let q = SqlQueryBuilder::new("SELECT * FROM job".to_string())
            .with_pagination_and_sorting(0, 10, None, None, "id", ALLOWED)
            .build();
        assert!(!q.contains("OFFSET"));
    }
}
