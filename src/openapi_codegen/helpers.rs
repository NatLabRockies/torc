use serde_json::{Map, Value};

pub fn check_operation_id(
    source: &str,
    emitted: &Value,
    path: &str,
    method: &str,
    expected: &str,
    issues: &mut Vec<String>,
) {
    let source_operation_id = source_operation_id(source, path, method);
    let emitted_operation_id = emitted
        .get("paths")
        .and_then(|paths| paths.get(path))
        .and_then(|path_item| path_item.get(method))
        .and_then(|op| op.get("operationId"))
        .and_then(Value::as_str);

    if source_operation_id != Some(expected) {
        issues.push(format!(
            "source operationId mismatch for {} {}: expected {}, found {:?}",
            method, path, expected, source_operation_id
        ));
    }

    if emitted_operation_id != Some(expected) {
        issues.push(format!(
            "emitted operationId mismatch for {} {}: expected {}, found {:?}",
            method, path, expected, emitted_operation_id
        ));
    }
}

pub fn check_schema_properties(
    emitted: &Value,
    path: &str,
    method: &str,
    expected_properties: &[&str],
    issues: &mut Vec<String>,
) {
    let schema = emitted
        .get("paths")
        .and_then(|paths| paths.get(path))
        .and_then(|path_item| path_item.get(method))
        .and_then(|op| op.get("responses"))
        .and_then(|responses| responses.get("200"))
        .and_then(|response| response.get("content"))
        .and_then(|content| content.get("application/json"))
        .and_then(|json_content| json_content.get("schema"));

    let Some(schema) = schema else {
        issues.push(format!(
            "emitted schema missing for {} {} 200 response",
            method, path
        ));
        return;
    };

    let properties = resolve_schema_properties(emitted, schema);
    let Some(properties) = properties else {
        issues.push(format!(
            "unable to resolve emitted properties for {} {}",
            method, path
        ));
        return;
    };

    for property in expected_properties {
        if !properties.contains_key(*property) {
            issues.push(format!(
                "emitted schema for {} {} missing property {}",
                method, path, property
            ));
        }
    }
}

pub fn check_component_properties(
    document: &Value,
    schema_name: &str,
    expected_properties: &[&str],
    issues: &mut Vec<String>,
) {
    let properties = document
        .get("components")
        .and_then(|components| components.get("schemas"))
        .and_then(|schemas| schemas.get(schema_name))
        .and_then(|schema| schema.get("properties"))
        .and_then(Value::as_object);

    let Some(properties) = properties else {
        issues.push(format!("emitted component schema missing: {schema_name}"));
        return;
    };

    for property in expected_properties {
        if !properties.contains_key(*property) {
            issues.push(format!(
                "emitted component {schema_name} missing property {}",
                property
            ));
        }
    }
}

fn source_operation_id<'a>(source: &'a str, path: &str, method: &str) -> Option<&'a str> {
    let path_line = format!("  {path}:");
    let start = source.find(&path_line)?;
    let remaining = &source[start..];
    let end = remaining[1..]
        .find("\n  /")
        .map(|index| index + 1)
        .unwrap_or(remaining.len());
    let section = &remaining[..end];

    let mut current_method: Option<&str> = None;

    for line in section.lines() {
        if let Some(method_name) = line
            .strip_prefix("    ")
            .and_then(|rest| rest.strip_suffix(':'))
            .filter(|value| matches!(*value, "get" | "post" | "put" | "delete" | "patch"))
        {
            current_method = Some(method_name);
            continue;
        }

        if current_method == Some(method)
            && let Some(value) = line.trim().strip_prefix("operationId: ")
        {
            return Some(value.trim());
        }
    }

    None
}

fn resolve_schema_properties<'a>(
    document: &'a Value,
    schema: &'a Value,
) -> Option<&'a Map<String, Value>> {
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        let schema_name = reference.rsplit('/').next()?;
        return document
            .get("components")
            .and_then(|components| components.get("schemas"))
            .and_then(|schemas| schemas.get(schema_name))
            .and_then(|schema| schema.get("properties"))
            .and_then(Value::as_object);
    }

    schema.get("properties").and_then(Value::as_object)
}
