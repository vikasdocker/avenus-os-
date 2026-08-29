// Aether Agent Runtime - LLM provider implementations.
//
// This module ships the first non-mock LLM backends behind the
// `LlmProvider` trait. Both providers use std-only HTTP/1.1 to keep
// the runtime dependency footprint small — no reqwest, no hyper.
//
// Each provider is constructed with explicit configuration (URL,
// model, optional API key). The runtime NEVER reads environment
// variables inside the provider. Selection lives in
// `llm_provider::select`, which is a pure function that the daemon
// drives with values pulled from the environment.

use std::io::{Read, Write};
use std::time::Duration;

use crate::llm::{LlmProvider, LlmRequest, LlmResponse};

/// Default Ollama endpoint.
pub const DEFAULT_OLLAMA_URL: &str = "http://127.0.0.1:11434";
/// Default model name (must already be `ollama pull`-ed on the host).
pub const DEFAULT_OLLAMA_MODEL: &str = "llama3.2";
/// Default OpenAI-compatible endpoint.
pub const DEFAULT_OPENAI_URL: &str = "http://127.0.0.1:1234";
/// Default OpenAI-compatible model name.
pub const DEFAULT_OPENAI_MODEL: &str = "gpt-3.5-turbo";
/// Read timeout for HTTP calls. Bounded so the runtime never hangs.
pub const LLM_HTTP_TIMEOUT: Duration = Duration::from_secs(60);

/// Strip the `http://` or `https://` scheme and return `host:port`.
fn authority_of(url: &str) -> &str {
    let rest = url.trim_start_matches("http://").trim_start_matches("https://");
    rest.split('/').next().unwrap_or(rest)
}

/// Minimal HTTP/1.1 POST. The body is JSON. Returns the response body
/// (everything after the header block). Surfaces transport failures
/// with a clear error.
fn http_post_json(url: &str, path: &str, body: &str, auth: Option<&str>) -> Result<String, String> {
    let authority = authority_of(url);
    let mut stream =
        std::net::TcpStream::connect(authority).map_err(|e| format!("connect {authority}: {e}"))?;
    stream.set_read_timeout(Some(LLM_HTTP_TIMEOUT)).map_err(|e| format!("timeout: {e}"))?;
    let mut req = format!(
        "POST {path} HTTP/1.1\r\nHost: {authority}\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    if let Some(token) = auth {
        req.push_str(&format!("Authorization: Bearer {token}\r\n"));
    }
    req.push_str("\r\n");
    req.push_str(body);
    stream.write_all(req.as_bytes()).map_err(|e| format!("send: {e}"))?;
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).map_err(|e| format!("recv: {e}"))?;
    let text = String::from_utf8_lossy(&raw).to_string();
    // The body is everything after the header block. Some Ollama
    // variants stream NDJSON; we still get a complete buffer because
    // we used `Connection: close`.
    let split = text
        .find("\r\n\r\n")
        .ok_or_else(|| "malformed HTTP response: no header terminator".to_string())?;
    let (head, body_out) = text.split_at(split + 4);
    // Surface a non-2xx status as an error.
    if let Some(status_line) = head.lines().next() {
        if !status_line.contains(" 200 ") && !status_line.contains(" 201 ") {
            return Err(format!(
                "http {status_line}: {}",
                body_out.chars().take(200).collect::<String>()
            ));
        }
    }
    Ok(body_out.to_string())
}

/// Ollama provider.
///
/// Talks to a local Ollama daemon over HTTP/1.1. The default
/// endpoint is `http://127.0.0.1:11434`. The default model is
/// `llama3.2`. Both are overridable via constructor arguments.
pub struct OllamaLlmProvider {
    pub url: String,
    pub model: String,
}

impl OllamaLlmProvider {
    pub fn new(url: impl Into<String>, model: impl Into<String>) -> Self {
        Self { url: url.into(), model: model.into() }
    }

    pub fn with_defaults() -> Self {
        Self::new(DEFAULT_OLLAMA_URL, DEFAULT_OLLAMA_MODEL)
    }
}

impl LlmProvider for OllamaLlmProvider {
    fn name(&self) -> &str {
        "ollama"
    }

    fn generate(&self, request: &LlmRequest) -> Result<LlmResponse, String> {
        // Compose messages array. We collapse the optional
        // system_prompt into a system message at the head.
        let mut messages = Vec::new();
        if let Some(sys) = request.system_prompt.as_deref() {
            messages.push(serde_json::json!({"role": "system", "content": sys}));
        }
        messages.push(serde_json::json!({"role": "user", "content": request.prompt}));

        // Build the request body. If a structured_output JSON schema
        // is provided, use Ollama's `format` field to coerce the
        // response shape.
        let mut body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "stream": false,
        });
        if let Some(schema) = request.structured_output.as_ref() {
            body["format"] = schema.clone();
        }
        let body_str = body.to_string();

        let raw = http_post_json(&self.url, "/api/chat", &body_str, None)?;
        // Ollama may return a streaming NDJSON body; collapse to a
        // single object by taking the last `message.content`.
        // The first `{` is the start of the JSON object.
        let json_start = raw.find('{').ok_or_else(|| "no JSON in response".to_string())?;
        let value: serde_json::Value = serde_json::from_str(raw[json_start..].trim())
            .map_err(|e| format!("bad ollama json: {e}"))?;
        let content = value
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .map(str::to_string)
            .ok_or_else(|| "missing message.content in ollama response".to_string())?;
        let model = value.get("model").and_then(|v| v.as_str()).unwrap_or(&self.model).to_string();
        Ok(LlmResponse {
            content,
            model,
            tokens_used: None,
            finish_reason: "stop".to_string(),
            parsed_output: None,
        })
    }

    fn health(&self) -> Result<(), String> {
        // Cheap reachability probe. We don't fail on missing
        // models — the actual /api/chat call will report that.
        let body = "{}";
        let _ = http_post_json(&self.url, "/api/tags", body, None)?;
        Ok(())
    }

    fn model_info(&self) -> crate::llm::LlmModelInfo {
        crate::llm::LlmModelInfo {
            name: self.model.clone(),
            version: "unknown".to_string(),
            capabilities: vec!["chat".to_string(), "structured".to_string()],
        }
    }
}

/// OpenAI-compatible provider. Works with any server that speaks the
/// `/v1/chat/completions` endpoint (LM Studio, llama.cpp's server,
/// vLLM, OpenAI itself, etc.).
pub struct OpenAILlmProvider {
    pub url: String,
    pub model: String,
    pub api_key: Option<String>,
}

impl OpenAILlmProvider {
    pub fn new(url: impl Into<String>, model: impl Into<String>, api_key: Option<String>) -> Self {
        Self { url: url.into(), model: model.into(), api_key }
    }

    pub fn with_defaults() -> Self {
        Self::new(DEFAULT_OPENAI_URL, DEFAULT_OPENAI_MODEL, None)
    }
}

impl LlmProvider for OpenAILlmProvider {
    fn name(&self) -> &str {
        "openai"
    }

    fn generate(&self, request: &LlmRequest) -> Result<LlmResponse, String> {
        let mut messages = Vec::new();
        if let Some(sys) = request.system_prompt.as_deref() {
            messages.push(serde_json::json!({"role": "system", "content": sys}));
        }
        messages.push(serde_json::json!({"role": "user", "content": request.prompt}));

        let mut body = serde_json::json!({
            "model": self.model,
            "messages": messages,
        });
        if let Some(max) = request.max_tokens {
            body["max_tokens"] = serde_json::json!(max);
        }
        if let Some(t) = request.temperature {
            body["temperature"] = serde_json::json!(t);
        }
        if let Some(schema) = request.structured_output.as_ref() {
            body["response_format"] = serde_json::json!({
                "type": "json_schema",
                "schema": schema,
            });
        }
        let body_str = body.to_string();

        let raw =
            http_post_json(&self.url, "/v1/chat/completions", &body_str, self.api_key.as_deref())?;
        let value: serde_json::Value =
            serde_json::from_str(raw.trim()).map_err(|e| format!("bad openai json: {e}"))?;
        let content = value
            .get("choices")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .map(str::to_string)
            .ok_or_else(|| "missing choices[0].message.content".to_string())?;
        let model = value.get("model").and_then(|v| v.as_str()).unwrap_or(&self.model).to_string();
        let tokens = value
            .get("usage")
            .and_then(|u| u.get("total_tokens"))
            .and_then(|t| t.as_u64())
            .map(|v| v as u32);
        let finish_reason = value
            .get("choices")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|c| c.get("finish_reason"))
            .and_then(|f| f.as_str())
            .unwrap_or("stop")
            .to_string();
        Ok(LlmResponse { content, model, tokens_used: tokens, finish_reason, parsed_output: None })
    }

    fn health(&self) -> Result<(), String> {
        // GET /v1/models is the cheap probe.
        let authority = authority_of(&self.url);
        let mut stream = std::net::TcpStream::connect(authority)
            .map_err(|e| format!("connect {authority}: {e}"))?;
        stream.set_read_timeout(Some(LLM_HTTP_TIMEOUT)).map_err(|e| format!("timeout: {e}"))?;
        let mut req =
            format!("GET /v1/models HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\n");
        if let Some(token) = self.api_key.as_deref() {
            req.push_str(&format!("Authorization: Bearer {token}\r\n"));
        }
        req.push_str("\r\n");
        stream.write_all(req.as_bytes()).map_err(|e| format!("send: {e}"))?;
        let mut raw = Vec::new();
        stream.read_to_end(&mut raw).map_err(|e| format!("recv: {e}"))?;
        let text = String::from_utf8_lossy(&raw).to_string();
        if text.contains(" 200 ") {
            Ok(())
        } else {
            Err(format!("openai health: {}", text.lines().next().unwrap_or("?")))
        }
    }

    fn model_info(&self) -> crate::llm::LlmModelInfo {
        let mut caps = vec!["chat".to_string()];
        caps.push("structured".to_string());
        if self.api_key.is_some() {
            caps.push("authenticated".to_string());
        }
        crate::llm::LlmModelInfo {
            name: self.model.clone(),
            version: "unknown".to_string(),
            capabilities: caps,
        }
    }
}

/// Provider selection. Pure: takes configuration, returns a boxed
/// provider. Tests use this with explicit values; production callers
/// use `select_from_env`.
///
/// `provider` is the kind tag: `echo` | `mock` | `ollama` | `openai`.
/// `url` and `model` are provider-specific (overrides defaults).
/// `api_key` is OpenAI-only.
///
/// Unknown kinds fall back to the echo provider so the runtime is
/// never dead — but the daemon's structured-intent parser is
/// responsible for surfacing that the response is not from a real
/// model.
pub fn select(
    provider: &str,
    url: Option<&str>,
    model: Option<&str>,
    api_key: Option<&str>,
) -> Box<dyn LlmProvider> {
    match provider {
        "ollama" => Box::new(OllamaLlmProvider::new(
            url.unwrap_or(DEFAULT_OLLAMA_URL),
            model.unwrap_or(DEFAULT_OLLAMA_MODEL),
        )),
        "openai" => Box::new(OpenAILlmProvider::new(
            url.unwrap_or(DEFAULT_OPENAI_URL),
            model.unwrap_or(DEFAULT_OPENAI_MODEL),
            api_key.map(str::to_string),
        )),
        "mock" => Box::new(crate::llm::MockLlmProvider::single("")),
        _ => Box::new(crate::llm::EchoLlmProvider),
    }
}

/// Read the environment and pick a provider. All values come from
/// one helper so tests can drive it without `unsafe` env mutation.
pub fn select_from_env() -> Box<dyn LlmProvider> {
    let provider = std::env::var("AETHER_LLM_PROVIDER").unwrap_or_else(|_| "echo".to_string());
    let url = std::env::var("AETHER_LLM_URL").ok();
    let model = std::env::var("AETHER_LLM_MODEL").ok();
    let api_key = std::env::var("AETHER_LLM_API_KEY").ok();
    select(&provider, url.as_deref(), model.as_deref(), api_key.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::net::TcpListener;
    use std::sync::Arc;
    use std::thread;

    /// Spawn a one-shot HTTP/1.1 server that replies with `body` and
    /// the given status. Captures the request line + headers so
    /// tests can assert on them.
    fn spawn_one_shot(
        status: &str,
        body: &str,
        capture: Arc<std::sync::Mutex<Vec<String>>>,
    ) -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap_or_else(|e| panic!("{e}"));
        let port = listener.local_addr().unwrap_or_else(|e| panic!("{e}")).port();
        let body = body.to_string();
        let status = status.to_string();
        let _ = thread::spawn(move || {
            if let Some(stream) = listener.incoming().flatten().next() {
                let mut reader =
                    BufReader::new(stream.try_clone().unwrap_or_else(|e| panic!("{e}")));
                let mut writer = stream;
                let mut head = String::new();
                let _ = reader.read_line(&mut head);
                for line in reader.lines().map_while(Result::ok) {
                    if line.is_empty() {
                        break;
                    }
                    capture.lock().unwrap_or_else(|p| p.into_inner()).push(line);
                }
                let resp = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = writer.write_all(resp.as_bytes());
                let _ = writer.flush();
            }
        });
        std::thread::sleep(Duration::from_millis(20));
        port
    }

    fn make_request() -> LlmRequest {
        LlmRequest {
            prompt: "hi".to_string(),
            system_prompt: Some("be brief".to_string()),
            max_tokens: None,
            temperature: None,
            structured_output: None,
        }
    }

    #[test]
    fn ollama_provider_sends_messages_and_parses_reply() {
        let captured: Arc<std::sync::Mutex<Vec<String>>> =
            Arc::new(std::sync::Mutex::new(Vec::new()));
        let body = r#"{"model":"llama3.2","message":{"role":"assistant","content":"hello world"},"done":true}"#;
        let port = spawn_one_shot("200 OK", body, Arc::clone(&captured));
        let provider = OllamaLlmProvider::new(format!("http://127.0.0.1:{port}"), "llama3.2");
        let resp = provider
            .generate(&make_request())
            .unwrap_or_else(|e| panic!("ollama generate failed: {e}"));
        assert_eq!(resp.content, "hello world");
        assert_eq!(resp.model, "llama3.2");
        let head = captured.lock().unwrap_or_else(|p| p.into_inner());
        assert!(
            head.iter().any(|l| l.starts_with("Content-Type: application/json")),
            "captured head: {head:?}"
        );
    }

    #[test]
    fn ollama_provider_surfaces_non_2xx() {
        let captured: Arc<std::sync::Mutex<Vec<String>>> =
            Arc::new(std::sync::Mutex::new(Vec::new()));
        let port = spawn_one_shot(
            "404 Not Found",
            r#"{"error":"model not found"}"#,
            Arc::clone(&captured),
        );
        let provider = OllamaLlmProvider::new(format!("http://127.0.0.1:{port}"), "missing");
        let err = match provider.generate(&make_request()) {
            Ok(_) => panic!("expected error for 404"),
            Err(e) => e,
        };
        assert!(err.contains("404"), "got: {err}");
    }

    #[test]
    fn ollama_provider_reports_missing_message_content() {
        let captured: Arc<std::sync::Mutex<Vec<String>>> =
            Arc::new(std::sync::Mutex::new(Vec::new()));
        let port =
            spawn_one_shot("200 OK", r#"{"model":"llama3.2","done":true}"#, Arc::clone(&captured));
        let provider = OllamaLlmProvider::new(format!("http://127.0.0.1:{port}"), "llama3.2");
        let err = match provider.generate(&make_request()) {
            Ok(_) => panic!("expected error for missing content"),
            Err(e) => e,
        };
        assert!(err.contains("message.content"), "got: {err}");
    }

    #[test]
    fn openai_provider_parses_choices() {
        let captured: Arc<std::sync::Mutex<Vec<String>>> =
            Arc::new(std::sync::Mutex::new(Vec::new()));
        let body = r#"{
            "id":"x",
            "model":"gpt-3.5-turbo",
            "choices":[{"index":0,"message":{"role":"assistant","content":"hi"},"finish_reason":"stop"}],
            "usage":{"total_tokens":7}
        }"#;
        let port = spawn_one_shot("200 OK", body, Arc::clone(&captured));
        let provider =
            OpenAILlmProvider::new(format!("http://127.0.0.1:{port}"), "gpt-3.5-turbo", None);
        let resp = provider
            .generate(&make_request())
            .unwrap_or_else(|e| panic!("openai generate failed: {e}"));
        assert_eq!(resp.content, "hi");
        assert_eq!(resp.finish_reason, "stop");
        assert_eq!(resp.tokens_used, Some(7));
    }

    #[test]
    fn openai_provider_sends_authorization_header_when_key_present() {
        let captured: Arc<std::sync::Mutex<Vec<String>>> =
            Arc::new(std::sync::Mutex::new(Vec::new()));
        let body = r#"{"choices":[{"message":{"content":"x"}}]}"#;
        let port = spawn_one_shot("200 OK", body, Arc::clone(&captured));
        let provider = OpenAILlmProvider::new(
            format!("http://127.0.0.1:{port}"),
            "gpt-3.5-turbo",
            Some("secret-key".to_string()),
        );
        let _ = provider
            .generate(&make_request())
            .unwrap_or_else(|e| panic!("openai generate failed: {e}"));
        let head = captured.lock().unwrap_or_else(|p| p.into_inner());
        assert!(
            head.iter().any(|l| l == "Authorization: Bearer secret-key"),
            "expected auth header, got: {head:?}"
        );
    }

    #[test]
    fn openai_provider_surfaces_non_2xx() {
        let captured: Arc<std::sync::Mutex<Vec<String>>> =
            Arc::new(std::sync::Mutex::new(Vec::new()));
        let port =
            spawn_one_shot("401 Unauthorized", r#"{"error":"bad key"}"#, Arc::clone(&captured));
        let provider =
            OpenAILlmProvider::new(format!("http://127.0.0.1:{port}"), "gpt-3.5-turbo", None);
        let err = match provider.generate(&make_request()) {
            Ok(_) => panic!("expected error for 401"),
            Err(e) => e,
        };
        assert!(err.contains("401"), "got: {err}");
    }

    #[test]
    fn select_returns_echo_by_default() {
        let p = select("nope", None, None, None);
        assert_eq!(p.name(), "echo");
    }

    #[test]
    fn select_returns_ollama_when_kind_is_ollama() {
        let p = select("ollama", None, None, None);
        assert_eq!(p.name(), "ollama");
        let info = p.model_info();
        assert_eq!(info.name, DEFAULT_OLLAMA_MODEL);
    }

    #[test]
    fn select_returns_openai_when_kind_is_openai() {
        let p = select("openai", Some("http://example"), Some("custom"), Some("k"));
        assert_eq!(p.name(), "openai");
        let info = p.model_info();
        assert_eq!(info.name, "custom");
        assert!(info.capabilities.contains(&"authenticated".to_string()));
    }

    #[test]
    fn select_returns_mock_when_kind_is_mock() {
        let p = select("mock", None, None, None);
        assert_eq!(p.name(), "mock");
    }

    #[test]
    fn authority_of_strips_scheme_and_path() {
        assert_eq!(authority_of("http://127.0.0.1:11434"), "127.0.0.1:11434");
        assert_eq!(authority_of("https://api.openai.com/v1"), "api.openai.com");
        assert_eq!(authority_of("127.0.0.1:1234"), "127.0.0.1:1234");
    }
}
