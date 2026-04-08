use anyhow::{anyhow, Result};

/// Strip markdown code fences if present (```json ... ``` or ``` ... ```)
fn strip_code_fences(s: &str) -> &str {
    let s = s.trim();
    // Try to find opening fence
    if let Some(rest) = s.strip_prefix("```") {
        // skip optional language tag on same line
        let after_lang = if let Some(nl) = rest.find('\n') {
            &rest[nl + 1..]
        } else {
            rest
        };
        // strip closing fence
        if let Some(end) = after_lang.rfind("```") {
            return after_lang[..end].trim();
        }
        return after_lang.trim();
    }
    s
}

/// Validate the LLM output string against the bee's output_schema.
/// Returns a parsed serde_json::Value on success.
pub fn validate_and_parse(
    llm_output: &str,
    schema: &serde_json::Value,
) -> Result<serde_json::Value> {
    let cleaned = strip_code_fences(llm_output);

    let value: serde_json::Value = serde_json::from_str(cleaned)
        .map_err(|e| anyhow!("LLM output is not valid JSON: {}\nOutput was:\n{}", e, cleaned))?;

    validate_against_schema(&value, schema)?;

    Ok(value)
}

/// Validate a JSON value against a simple schema.
/// The schema can be:
/// - An object with field names as keys and type strings as values (simple schema)
/// - A JSON Schema object with "type" and "properties" / "required" keys
fn validate_against_schema(value: &serde_json::Value, schema: &serde_json::Value) -> Result<()> {
    match schema {
        serde_json::Value::Object(schema_obj) => {
            // Check if this looks like a JSON Schema (has "type" or "properties" key)
            if schema_obj.contains_key("properties") || schema_obj.contains_key("required") {
                validate_json_schema(value, schema)
            } else {
                // Simple schema: {"field_name": "type_string"}
                validate_simple_schema(value, schema_obj)
            }
        }
        _ => Ok(()), // No schema constraints
    }
}

/// Validate against a simple {"field": "type"} schema
fn validate_simple_schema(
    value: &serde_json::Value,
    schema_obj: &serde_json::Map<String, serde_json::Value>,
) -> Result<()> {
    let obj = value.as_object().ok_or_else(|| anyhow!("Expected a JSON object, got: {}", value))?;

    for (field, type_val) in schema_obj {
        let type_str = type_val.as_str().unwrap_or("string");
        match obj.get(field) {
            None => {
                return Err(anyhow!("Missing required field: '{}'", field));
            }
            Some(v) => {
                validate_type(field, v, type_str)?;
            }
        }
    }
    Ok(())
}

/// Validate a value's type
fn validate_type(field: &str, value: &serde_json::Value, expected_type: &str) -> Result<()> {
    match expected_type {
        "string" => {
            if !value.is_string() {
                return Err(anyhow!("Field '{}' should be a string, got: {}", field, value));
            }
        }
        "integer" | "number" => {
            if !value.is_number() {
                return Err(anyhow!("Field '{}' should be a number, got: {}", field, value));
            }
        }
        "boolean" => {
            if !value.is_boolean() {
                return Err(anyhow!("Field '{}' should be a boolean, got: {}", field, value));
            }
        }
        "array" => {
            if !value.is_array() {
                return Err(anyhow!("Field '{}' should be an array, got: {}", field, value));
            }
        }
        "object" => {
            if !value.is_object() {
                return Err(anyhow!("Field '{}' should be an object, got: {}", field, value));
            }
        }
        _ => {} // Unknown type, skip validation
    }
    Ok(())
}

/// Basic JSON Schema validation (supports "properties" and "required")
fn validate_json_schema(value: &serde_json::Value, schema: &serde_json::Value) -> Result<()> {
    let obj = value.as_object().ok_or_else(|| anyhow!("Expected a JSON object"))?;

    // Check required fields
    if let Some(required) = schema.get("required").and_then(|r| r.as_array()) {
        for req in required {
            if let Some(field) = req.as_str() {
                if !obj.contains_key(field) {
                    return Err(anyhow!("Missing required field: '{}'", field));
                }
            }
        }
    }

    // Validate properties
    if let Some(props) = schema.get("properties").and_then(|p| p.as_object()) {
        for (field, field_schema) in props {
            if let Some(v) = obj.get(field) {
                if let Some(type_str) = field_schema.get("type").and_then(|t| t.as_str()) {
                    validate_type(field, v, type_str)?;
                }
            }
        }
    }

    Ok(())
}
