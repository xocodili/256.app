//! Canonical JSON (sorted object keys at every depth) and SHA-256 hex for `job_queue.payload_hash`.

use std::collections::BTreeMap;

use serde_json::Value;
use sha2::{Digest, Sha256};

/// Recursively sort object keys so logically identical maps hash identically.
pub fn sort_json_value(v: &Value) -> Value {
    match v {
        Value::Object(map) => {
            let sorted: BTreeMap<String, Value> = map
                .iter()
                .map(|(k, v)| (k.clone(), sort_json_value(v)))
                .collect();
            Value::Object(sorted.into_iter().collect())
        }
        Value::Array(items) => Value::Array(items.iter().map(sort_json_value).collect()),
        x => x.clone(),
    }
}

pub fn canonical_json_string(payload: &Value) -> String {
    let sorted = sort_json_value(payload);
    serde_json::to_string(&sorted).unwrap_or_else(|_| "{}".to_string())
}

/// Lowercase hex SHA-256 of [`canonical_json_string`].
pub fn payload_hash(payload: &Value) -> String {
    let s = canonical_json_string(payload);
    let digest = Sha256::digest(s.as_bytes());
    hex::encode(digest)
}
