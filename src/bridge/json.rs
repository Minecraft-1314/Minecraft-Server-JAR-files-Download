use serde_json::{json, Value};

pub fn text(source: &Value, name: &str, fallback: &str) -> String {
    source
        .get(name)
        .and_then(Value::as_str)
        .map(|value| value.to_string())
        .unwrap_or_else(|| fallback.to_string())
}

pub fn integer(source: &Value, name: &str, fallback: i32) -> i32 {
    match source.get(name) {
        Some(Value::Number(number)) => number
            .as_i64()
            .map(|value| value as i32)
            .unwrap_or(fallback),
        Some(Value::String(value)) => value.parse::<i32>().unwrap_or(fallback),
        _ => fallback,
    }
}

pub fn long(source: &Value, name: &str) -> Option<i64> {
    match source.get(name) {
        Some(Value::Number(number)) => number.as_i64(),
        Some(Value::String(value)) => value.parse::<i64>().ok(),
        _ => None,
    }
}

pub fn boolean(source: &Value, name: &str, fallback: bool) -> bool {
    match source.get(name) {
        Some(Value::Bool(value)) => *value,
        Some(Value::String(value)) => value.parse::<bool>().unwrap_or(fallback),
        _ => fallback,
    }
}

pub fn object(source: &Value, name: &str) -> Value {
    source.get(name).cloned().unwrap_or(Value::Null)
}

pub fn error_payload(code: &str, fields: Vec<String>) -> Value {
    json!({ "code": code, "fields": fields })
}
