use serde_json::Value;
use std::{
    env,
    io::{self, Read},
};

pub fn read_stdin_json() -> Option<Value> {
    let mut input = String::new();
    if io::stdin().read_to_string(&mut input).is_err() {
        return None;
    }
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    serde_json::from_str(trimmed).ok()
}

pub fn model(input: Option<&Value>) -> String {
    if let Ok(val) = env::var("CC_MODEL") {
        if !val.is_empty() {
            return val;
        }
    }

    input
        .and_then(|root| {
            lookup_string(root, &["model", "display_name"])
                .or_else(|| lookup_string(root, &["model", "id"]))
        })
        .unwrap_or_else(|| "-".to_string())
}

pub fn version(input: Option<&Value>) -> Option<String> {
    if let Ok(val) = env::var("CC_VERSION") {
        if !val.is_empty() {
            return Some(val);
        }
    }

    input.and_then(|root| lookup_string(root, &["version"]))
}

pub fn lookup_string(root: &Value, path: &[&str]) -> Option<String> {
    let value = lookup_value(root, path)?;
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

pub fn lookup_u64(root: &Value, path: &[&str]) -> Option<u64> {
    let value = lookup_value(root, path)?;
    match value {
        Value::Number(n) => n.as_u64(),
        Value::String(s) => s.parse::<u64>().ok(),
        _ => None,
    }
}

fn lookup_value<'a>(root: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = root;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}
