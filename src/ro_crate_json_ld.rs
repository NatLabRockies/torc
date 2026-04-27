pub fn typed_entity(primary_type: &str, prov_type: &str) -> serde_json::Value {
    serde_json::json!([primary_type, prov_type])
}
