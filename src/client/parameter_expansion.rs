use std::collections::HashMap;

/// Represents a single parameter value (integer, float, or string)
#[derive(Clone, Debug, PartialEq)]
pub enum ParameterValue {
    Integer(i64),
    Float(f64),
    String(String),
}

impl std::fmt::Display for ParameterValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParameterValue::Integer(i) => write!(f, "{}", i),
            ParameterValue::Float(fl) => write!(f, "{}", fl),
            ParameterValue::String(s) => write!(f, "{}", s),
        }
    }
}

impl ParameterValue {
    /// Format the parameter value with optional format specifier
    /// Supports printf-style format specifiers like {:03d} for integers
    pub fn format(&self, format_spec: Option<&str>) -> String {
        match (self, format_spec) {
            (ParameterValue::Integer(i), Some(spec)) => {
                // Parse format spec like "03d" to mean zero-padded 3 digits
                if let Some(width_str) = spec.strip_suffix('d')
                    && let Some(width_str) = width_str.strip_prefix('0')
                    && let Ok(width) = width_str.parse::<usize>()
                {
                    return format!("{:0width$}", i, width = width);
                }
                i.to_string()
            }
            (ParameterValue::Float(f), Some(spec)) => {
                // Parse format spec like ".2f" to mean 2 decimal places
                if let Some(precision_str) = spec.strip_suffix('f')
                    && let Some(precision_str) = precision_str.strip_prefix('.')
                    && let Ok(precision) = precision_str.parse::<usize>()
                {
                    return format!("{:.precision$}", f, precision = precision);
                }
                f.to_string()
            }
            _ => self.to_string(),
        }
    }
}

/// Parse a parameter value string into a vector of ParameterValues
/// Supports:
/// - Integer ranges: "1:100" (inclusive), "1:100:5" (with step)
/// - Float ranges: "0.0:1.0:0.1"
/// - Lists: "[1,5,10,50,100]" or "['train','test','validation']"
/// - File-backed lists: "@path/to/file.txt" (one value per line; blank lines and '#' comments skipped)
///
/// Also tolerates curly braces around values (e.g., "{1:100}" is treated as "1:100")
/// since users sometimes confuse parameter value syntax with template substitution syntax.
pub fn parse_parameter_value(value: &str) -> Result<Vec<ParameterValue>, String> {
    let trimmed = value.trim();

    // File-backed list: `@path/to/file.txt` (one value per line)
    if let Some(path) = trimmed.strip_prefix('@') {
        return parse_file_list(path);
    }

    // Strip curly braces if they wrap the entire value
    // This handles the common mistake of using {1:100} instead of 1:100
    // (users confuse parameter values with template substitution syntax like {index})
    let trimmed = if trimmed.starts_with('{') && trimmed.ends_with('}') && !trimmed.contains(',') {
        // Only strip if it looks like a wrapped range, not a JSON object
        trimmed
            .strip_prefix('{')
            .and_then(|s| s.strip_suffix('}'))
            .unwrap_or(trimmed)
            .trim()
    } else {
        trimmed
    };

    // Check for list notation
    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        return parse_list(trimmed);
    }

    // Check for range notation
    if trimmed.contains(':') {
        return parse_range(trimmed);
    }

    // Single value
    if let Ok(i) = trimmed.parse::<i64>() {
        return Ok(vec![ParameterValue::Integer(i)]);
    }
    if let Ok(f) = trimmed.parse::<f64>() {
        return Ok(vec![ParameterValue::Float(f)]);
    }

    // Treat as string
    Ok(vec![ParameterValue::String(trimmed.to_string())])
}

/// Parse a list notation like "[1,5,10]" or "['a','b','c']"
fn parse_list(value: &str) -> Result<Vec<ParameterValue>, String> {
    let inner = value.trim_start_matches('[').trim_end_matches(']').trim();

    if inner.is_empty() {
        return Ok(vec![]);
    }

    let mut values = Vec::new();
    for item in inner.split(',') {
        let item = item.trim();

        // Try to parse as integer first
        if let Ok(i) = item.parse::<i64>() {
            values.push(ParameterValue::Integer(i));
            continue;
        }

        // Try to parse as float
        if let Ok(f) = item.parse::<f64>() {
            values.push(ParameterValue::Float(f));
            continue;
        }

        // Handle quoted strings
        let unquoted = item
            .trim_start_matches('\'')
            .trim_end_matches('\'')
            .trim_start_matches('"')
            .trim_end_matches('"');

        values.push(ParameterValue::String(unquoted.to_string()));
    }

    Ok(values)
}

/// Parse a file containing one parameter value per line.
/// Lines are trimmed; empty lines and lines starting with '#' are skipped.
/// Each remaining line is parsed as Integer → Float → String, like list entries.
fn parse_file_list(path: &str) -> Result<Vec<ParameterValue>, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("Empty file path after '@' in parameter value".to_string());
    }
    let content = std::fs::read_to_string(trimmed)
        .map_err(|e| format!("Failed to read parameter file '{}': {}", trimmed, e))?;

    let mut values = Vec::new();
    for raw in content.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Ok(i) = line.parse::<i64>() {
            values.push(ParameterValue::Integer(i));
        } else if let Ok(f) = line.parse::<f64>() {
            values.push(ParameterValue::Float(f));
        } else {
            values.push(ParameterValue::String(line.to_string()));
        }
    }
    Ok(values)
}

/// Parse a range notation like "1:100" or "0.0:1.0:0.1"
fn parse_range(value: &str) -> Result<Vec<ParameterValue>, String> {
    let parts: Vec<&str> = value.split(':').collect();

    if parts.len() < 2 || parts.len() > 3 {
        return Err(format!(
            "Invalid range format: '{}'. Expected 'start:end' or 'start:end:step'",
            value
        ));
    }

    let start_str = parts[0].trim();
    let end_str = parts[1].trim();
    let step_str = if parts.len() == 3 {
        parts[2].trim()
    } else {
        ""
    };

    // Try to parse as integers
    if let (Ok(start), Ok(end)) = (start_str.parse::<i64>(), end_str.parse::<i64>()) {
        let step = if !step_str.is_empty() {
            step_str
                .parse::<i64>()
                .map_err(|_| format!("Invalid integer step in range: '{}'", step_str))?
        } else {
            1
        };

        if step == 0 {
            return Err("Step cannot be zero".to_string());
        }

        let mut values = Vec::new();
        if step > 0 {
            let mut current = start;
            while current <= end {
                values.push(ParameterValue::Integer(current));
                current += step;
            }
        } else {
            let mut current = start;
            while current >= end {
                values.push(ParameterValue::Integer(current));
                current += step;
            }
        }

        return Ok(values);
    }

    // Try to parse as floats
    if let (Ok(start), Ok(end)) = (start_str.parse::<f64>(), end_str.parse::<f64>()) {
        let step = if !step_str.is_empty() {
            step_str
                .parse::<f64>()
                .map_err(|_| format!("Invalid float step in range: '{}'", step_str))?
        } else {
            1.0
        };

        if step == 0.0 {
            return Err("Step cannot be zero".to_string());
        }

        let mut values = Vec::new();
        if step > 0.0 {
            let mut current = start;
            // Use epsilon comparison for floats to handle rounding errors
            while current <= end + 1e-10 {
                values.push(ParameterValue::Float(current));
                current += step;
            }
        } else {
            let mut current = start;
            while current >= end - 1e-10 {
                values.push(ParameterValue::Float(current));
                current += step;
            }
        }

        return Ok(values);
    }

    Err(format!(
        "Invalid range values: '{}'. Could not parse as integer or float range",
        value
    ))
}

/// Generate the Cartesian product of parameter values
/// Given a map of parameter names to value lists, returns a vector of all possible combinations
pub fn cartesian_product(
    params: &HashMap<String, Vec<ParameterValue>>,
) -> Vec<HashMap<String, ParameterValue>> {
    if params.is_empty() {
        return vec![HashMap::new()];
    }

    // Convert HashMap to Vec for consistent ordering
    let param_vec: Vec<(&String, &Vec<ParameterValue>)> = params.iter().collect();

    let mut result = vec![HashMap::new()];

    for (param_name, param_values) in param_vec {
        let mut new_result = Vec::new();
        for existing_combo in &result {
            for value in param_values {
                let mut new_combo = existing_combo.clone();
                new_combo.insert(param_name.clone(), value.clone());
                new_result.push(new_combo);
            }
        }
        result = new_result;
    }

    result
}

/// Zip parameter values together (like Python's zip function)
/// All parameter lists must have the same length
/// Given a map of parameter names to value lists, returns a vector where
/// the i-th element contains the i-th value from each parameter
pub fn zip_parameters(
    params: &HashMap<String, Vec<ParameterValue>>,
) -> Result<Vec<HashMap<String, ParameterValue>>, String> {
    if params.is_empty() {
        return Ok(vec![HashMap::new()]);
    }

    // Check that all parameter lists have the same length
    let lengths: Vec<(&String, usize)> = params.iter().map(|(k, v)| (k, v.len())).collect();
    let first_len = lengths[0].1;

    for (name, len) in &lengths {
        if *len != first_len {
            return Err(format!(
                "All parameters must have the same number of values when using 'zip' mode. \
                 Parameter '{}' has {} values, but '{}' has {} values.",
                lengths[0].0, first_len, name, len
            ));
        }
    }

    if first_len == 0 {
        return Ok(vec![]);
    }

    // Convert HashMap to Vec for consistent ordering
    let param_vec: Vec<(&String, &Vec<ParameterValue>)> = params.iter().collect();

    // Zip the values together
    let mut result = Vec::with_capacity(first_len);
    for i in 0..first_len {
        let mut combo = HashMap::new();
        for (param_name, param_values) in &param_vec {
            combo.insert((*param_name).clone(), param_values[i].clone());
        }
        result.push(combo);
    }

    Ok(result)
}

/// Substitute parameter values into a template string
/// Supports both {param_name} and {param_name:format} syntax
pub fn substitute_parameters(template: &str, params: &HashMap<String, ParameterValue>) -> String {
    let mut result = template.to_string();

    for (param_name, param_value) in params {
        // Look for {param_name:format} pattern
        let pattern_with_format = format!("{{{}:", param_name);
        if let Some(start_idx) = result.find(&pattern_with_format) {
            // Find the closing brace
            if let Some(end_idx) = result[start_idx..].find('}') {
                let full_pattern = &result[start_idx..start_idx + end_idx + 1];
                // Extract format specifier
                let format_spec = &full_pattern[pattern_with_format.len()..full_pattern.len() - 1];
                let replacement = param_value.format(Some(format_spec));
                result = result.replace(full_pattern, &replacement);
                continue;
            }
        }

        // Look for simple {param_name} pattern
        let pattern = format!("{{{}}}", param_name);
        result = result.replace(&pattern, &param_value.to_string());
    }

    result
}

/// Substitute parameter values into a regex pattern string
/// Escapes regex metacharacters in the parameter values to ensure literal matching
/// Supports both {param_name} and {param_name:format} syntax
pub fn substitute_parameters_regex(
    template: &str,
    params: &HashMap<String, ParameterValue>,
) -> String {
    let mut result = template.to_string();

    for (param_name, param_value) in params {
        // Look for {param_name:format} pattern
        let pattern_with_format = format!("{{{}:", param_name);
        if let Some(start_idx) = result.find(&pattern_with_format) {
            // Find the closing brace
            if let Some(end_idx) = result[start_idx..].find('}') {
                let full_pattern = &result[start_idx..start_idx + end_idx + 1];
                // Extract format specifier
                let format_spec = &full_pattern[pattern_with_format.len()..full_pattern.len() - 1];
                let value_str = param_value.format(Some(format_spec));
                let escaped = regex::escape(&value_str);
                result = result.replace(full_pattern, &escaped);
                continue;
            }
        }

        // Look for simple {param_name} pattern
        let pattern = format!("{{{}}}", param_name);
        let value_str = param_value.to_string();
        let escaped = regex::escape(&value_str);
        result = result.replace(&pattern, &escaped);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_integer_range() {
        let values = parse_parameter_value("1:5").unwrap();
        assert_eq!(values.len(), 5);
        assert_eq!(values[0], ParameterValue::Integer(1));
        assert_eq!(values[4], ParameterValue::Integer(5));
    }

    #[test]
    fn test_parse_integer_range_with_curly_braces() {
        // Users sometimes confuse parameter values with template substitution syntax
        // e.g., writing {1:1000} instead of 1:1000
        let values = parse_parameter_value("{1:5}").unwrap();
        assert_eq!(values.len(), 5);
        assert_eq!(values[0], ParameterValue::Integer(1));
        assert_eq!(values[4], ParameterValue::Integer(5));

        // Should also work with spaces
        let values = parse_parameter_value("{ 1:100 }").unwrap();
        assert_eq!(values.len(), 100);
    }

    #[test]
    fn test_parse_integer_range_with_step() {
        let values = parse_parameter_value("0:10:2").unwrap();
        assert_eq!(values.len(), 6);
        assert_eq!(values[0], ParameterValue::Integer(0));
        assert_eq!(values[5], ParameterValue::Integer(10));
    }

    #[test]
    fn test_parse_float_range() {
        let values = parse_parameter_value("0.0:1.0:0.5").unwrap();
        assert_eq!(values.len(), 3);
        assert_eq!(values[0], ParameterValue::Float(0.0));
        assert_eq!(values[2], ParameterValue::Float(1.0));
    }

    #[test]
    fn test_parse_integer_list() {
        let values = parse_parameter_value("[1,5,10,50,100]").unwrap();
        assert_eq!(values.len(), 5);
        assert_eq!(values[0], ParameterValue::Integer(1));
        assert_eq!(values[4], ParameterValue::Integer(100));
    }

    #[test]
    fn test_parse_string_list() {
        let values = parse_parameter_value("['train','test','validation']").unwrap();
        assert_eq!(values.len(), 3);
        assert_eq!(values[0], ParameterValue::String("train".to_string()));
        assert_eq!(values[2], ParameterValue::String("validation".to_string()));
    }

    #[test]
    fn test_parse_file_list_mixed_types() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("params.txt");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "# leading comment").unwrap();
        writeln!(f).unwrap();
        writeln!(f, "1").unwrap();
        writeln!(f, "2.5").unwrap();
        writeln!(f, "  alpha  ").unwrap();
        writeln!(f, "# trailing comment").unwrap();

        let arg = format!("@{}", path.display());
        let values = parse_parameter_value(&arg).unwrap();
        assert_eq!(values.len(), 3);
        assert_eq!(values[0], ParameterValue::Integer(1));
        assert_eq!(values[1], ParameterValue::Float(2.5));
        assert_eq!(values[2], ParameterValue::String("alpha".to_string()));
    }

    #[test]
    fn test_parse_file_list_missing_file() {
        let result = parse_parameter_value("@/definitely/not/a/real/path.txt");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_file_list_empty_path() {
        let result = parse_parameter_value("@");
        assert!(result.is_err());
    }

    #[test]
    fn test_cartesian_product() {
        let mut params = HashMap::new();
        params.insert(
            "a".to_string(),
            vec![ParameterValue::Integer(1), ParameterValue::Integer(2)],
        );
        params.insert(
            "b".to_string(),
            vec![
                ParameterValue::String("x".to_string()),
                ParameterValue::String("y".to_string()),
            ],
        );

        let result = cartesian_product(&params);
        assert_eq!(result.len(), 4); // 2 * 2 = 4 combinations
    }

    #[test]
    fn test_substitute_parameters() {
        let mut params = HashMap::new();
        params.insert("i".to_string(), ParameterValue::Integer(42));
        params.insert(
            "name".to_string(),
            ParameterValue::String("test".to_string()),
        );

        let result = substitute_parameters("job_{i}_{name}", &params);
        assert_eq!(result, "job_42_test");
    }

    #[test]
    fn test_substitute_with_format() {
        let mut params = HashMap::new();
        params.insert("i".to_string(), ParameterValue::Integer(5));

        let result = substitute_parameters("job_{i:03d}", &params);
        assert_eq!(result, "job_005");
    }

    #[test]
    fn test_format_float() {
        let value = ParameterValue::Float(1.23456);
        assert_eq!(value.format(Some(".2f")), "1.23");
    }

    #[test]
    fn test_zip_parameters_function() {
        let mut params = HashMap::new();
        params.insert(
            "dataset".to_string(),
            vec![
                ParameterValue::String("cifar10".to_string()),
                ParameterValue::String("mnist".to_string()),
                ParameterValue::String("imagenet".to_string()),
            ],
        );
        params.insert(
            "model".to_string(),
            vec![
                ParameterValue::String("resnet".to_string()),
                ParameterValue::String("vgg".to_string()),
                ParameterValue::String("transformer".to_string()),
            ],
        );

        let result = zip_parameters(&params).unwrap();
        assert_eq!(result.len(), 3); // 3 zipped pairs, not 9 combinations

        // Verify each combination has both parameters
        for combo in &result {
            assert!(combo.contains_key("dataset"));
            assert!(combo.contains_key("model"));
        }
    }

    #[test]
    fn test_zip_parameters_empty() {
        let params: HashMap<String, Vec<ParameterValue>> = HashMap::new();
        let result = zip_parameters(&params).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].is_empty());
    }

    #[test]
    fn test_zip_parameters_single_param() {
        let mut params = HashMap::new();
        params.insert(
            "i".to_string(),
            vec![
                ParameterValue::Integer(1),
                ParameterValue::Integer(2),
                ParameterValue::Integer(3),
            ],
        );

        let result = zip_parameters(&params).unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].get("i"), Some(&ParameterValue::Integer(1)));
        assert_eq!(result[1].get("i"), Some(&ParameterValue::Integer(2)));
        assert_eq!(result[2].get("i"), Some(&ParameterValue::Integer(3)));
    }
}
