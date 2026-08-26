// Output formatter for different output modes
use serde_json::Value;
use anyhow::Result;

pub struct OutputFormatter {
    json_mode: bool,
}

impl OutputFormatter {
    pub fn new() -> Self {
        Self {
            json_mode: std::env::var("AETHER_JSON_OUTPUT")
                .map(|v| v == "1" || v.to_lowercase() == "true")
                .unwrap_or(false),
        }
    }

    pub fn set_json_mode(&mut self, json: bool) {
        self.json_mode = json;
    }

    pub fn output(&self, data: &Value) -> Result<()> {
        if self.json_mode {
            println!("{}", serde_json::to_string_pretty(data)?);
        } else {
            println!("{}", self.format_human_readable(data));
        }
        Ok(())
    }

    pub fn print(&self, text: &str) {
        if !self.json_mode {
            println!("{}", text);
        }
    }

    pub fn error(&self, message: &str, code: &str, service: Option<&str>) -> Result<()> {
        let error_json = serde_json::json!({
            "error": {
                "code": code,
                "message": message,
                "service": service,
            }
        });

        if self.json_mode {
            println!("{}", serde_json::to_string_pretty(&error_json)?);
        } else {
            println!("\nERROR\nCode: {}\nMessage: {}\n", code, message);
            if let Some(svc) = service {
                println!("Service: {}\n", svc);
            }
        }
        Ok(())
    }

    fn format_human_readable(&self, data: &Value) -> String {
        match data {
            Value::Object(map) => {
                let mut output = String::new();
                for (key, value) in map {
                    output.push_str(&format!("{}: ", key));
                    match value {
                        Value::Array(arr) => {
                            output.push_str(&format!("{} items\n", arr.len()));
                        }
                        Value::Object(_) => {
                            output.push_str("(object)\n");
                        }
                        _ => {
                            output.push_str(&format!("{}\n", value));
                        }
                    }
                }
                output
            }
            Value::Array(arr) => {
                format!("{} items", arr.len())
            }
            _ => format!("{}", data),
        }
    }
}

impl Default for OutputFormatter {
    fn default() -> Self {
        Self::new()
    }
}
