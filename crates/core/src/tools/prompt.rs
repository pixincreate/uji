use serde_json::Value;

pub fn listing(arguments: &Value) -> String {
    match arguments {
        Value::Object(map) if !map.is_empty() => map
            .iter()
            .map(|(key, value)| {
                let value = value
                    .as_str()
                    .map_or_else(|| value.to_string(), str::to_string);
                format!("{key}: {value}")
            })
            .collect::<Vec<_>>()
            .join("\n"),
        other => other.to_string(),
    }
}
