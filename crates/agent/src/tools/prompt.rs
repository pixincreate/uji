use serde_json::{Map, Value};

pub struct Prompt {
    pub question: String,
    pub detail: String,
}

fn fields(arguments: &str) -> Map<String, Value> {
    match serde_json::from_str::<Value>(arguments) {
        Ok(Value::Object(map)) => map,
        _ => Map::new(),
    }
}

fn text(map: &Map<String, Value>, key: &str) -> String {
    map.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn listing(map: &Map<String, Value>) -> String {
    map.iter()
        .map(|(key, value)| {
            let value = value
                .as_str()
                .map_or_else(|| value.to_string(), str::to_string);
            format!("{key}: {value}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn describe(name: &str, arguments: &str) -> Prompt {
    let map = fields(arguments);
    let (question, detail) = match name {
        "run_command" => (
            "Would you like to run the following command?".to_string(),
            format!("$ {}", text(&map, "command")),
        ),
        "edit_file" => (
            "Would you like to make the following edit?".to_string(),
            format!("Destination: {}", text(&map, "path")),
        ),
        "write_file" => (
            "Would you like to write the following file?".to_string(),
            format!("Destination: {}", text(&map, "path")),
        ),
        "read_file" => (
            format!("Would you like to allow uji to `{name}`?"),
            format!("Path: {}", text(&map, "path")),
        ),
        _ => {
            let detail = if map.is_empty() {
                arguments.to_string()
            } else {
                listing(&map)
            };
            (format!("Would you like to run `{name}`?"), detail)
        }
    };
    Prompt { question, detail }
}
