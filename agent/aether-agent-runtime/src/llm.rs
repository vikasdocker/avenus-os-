// Agent Runtime - LLM Provider abstraction
//
// The Agent Runtime must not be tightly coupled to any specific LLM.
// Providers: Ollama, OpenAI-compatible, local models, future Aether backend.

use serde::{Deserialize, Serialize};

/// Request to an LLM provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRequest {
    pub prompt: String,
    pub system_prompt: Option<String>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub structured_output: Option<serde_json::Value>,
}

/// Response from an LLM provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    pub content: String,
    pub model: String,
    pub tokens_used: Option<u32>,
    pub finish_reason: String,
    pub parsed_output: Option<serde_json::Value>,
}

/// LLM provider trait.
pub trait LlmProvider: Send + Sync {
    fn name(&self) -> &str;

    /// Generate a response from a prompt.
    fn generate(&self, request: &LlmRequest) -> Result<LlmResponse, String>;

    /// Stream a response (optional, default blocks).
    fn stream(&self, request: &LlmRequest) -> Result<LlmResponse, String> {
        self.generate(request)
    }

    /// Generate structured output conforming to a schema.
    fn structured_output(
        &self,
        request: &LlmRequest,
        _schema: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let response = self.generate(request)?;
        serde_json::from_str(&response.content)
            .map_err(|e| format!("Failed to parse structured output: {e}"))
    }

    /// Health check.
    fn health(&self) -> Result<(), String> {
        Ok(())
    }

    /// Model information.
    fn model_info(&self) -> LlmModelInfo {
        LlmModelInfo {
            name: self.name().to_string(),
            version: "unknown".to_string(),
            capabilities: Vec::new(),
        }
    }
}

/// Information about an LLM model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmModelInfo {
    pub name: String,
    pub version: String,
    pub capabilities: Vec<String>,
}

/// Mock LLM provider for deterministic testing.
pub struct MockLlmProvider {
    responses: Vec<String>,
    index: std::sync::atomic::AtomicUsize,
}

impl MockLlmProvider {
    pub fn new(responses: Vec<String>) -> Self {
        Self {
            responses,
            index: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    pub fn single(response: &str) -> Self {
        Self::new(vec![response.to_string()])
    }
}

impl LlmProvider for MockLlmProvider {
    fn name(&self) -> &str {
        "mock"
    }

    fn generate(&self, _request: &LlmRequest) -> Result<LlmResponse, String> {
        let idx = self
            .index
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let content = self
            .responses
            .get(idx % self.responses.len())
            .cloned()
            .unwrap_or_default();
        Ok(LlmResponse {
            content,
            model: "mock".to_string(),
            tokens_used: Some(0),
            finish_reason: "stop".to_string(),
            parsed_output: None,
        })
    }

    fn health(&self) -> Result<(), String> {
        Ok(())
    }
}

/// Echo provider that returns the input as output (for development).
pub struct EchoLlmProvider;

impl LlmProvider for EchoLlmProvider {
    fn name(&self) -> &str {
        "echo"
    }

    fn generate(&self, request: &LlmRequest) -> Result<LlmResponse, String> {
        Ok(LlmResponse {
            content: request.prompt.clone(),
            model: "echo".to_string(),
            tokens_used: Some(0),
            finish_reason: "stop".to_string(),
            parsed_output: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_provider_returns_predefined_responses() {
        let p = MockLlmProvider::new(vec!["hello".to_string(), "world".to_string()]);
        let r1 = match p.generate(&LlmRequest {
            prompt: "q1".to_string(),
            system_prompt: None,
            max_tokens: None,
            temperature: None,
            structured_output: None,
        }) {
            Ok(v) => v,
            Err(e) => panic!("generate q1 failed: {e}"),
        };
        assert_eq!(r1.content, "hello");
        let r2 = match p.generate(&LlmRequest {
            prompt: "q2".to_string(),
            system_prompt: None,
            max_tokens: None,
            temperature: None,
            structured_output: None,
        }) {
            Ok(v) => v,
            Err(e) => panic!("generate q2 failed: {e}"),
        };
        assert_eq!(r2.content, "world");
    }

    #[test]
    fn mock_provider_wraps_around() {
        let p = MockLlmProvider::single("only");
        let r = match p.generate(&LlmRequest {
            prompt: "x".to_string(),
            system_prompt: None,
            max_tokens: None,
            temperature: None,
            structured_output: None,
        }) {
            Ok(v) => v,
            Err(e) => panic!("generate failed: {e}"),
        };
        assert_eq!(r.content, "only");
    }

    #[test]
    fn echo_provider_echoes() {
        let p = EchoLlmProvider;
        let r = match p.generate(&LlmRequest {
            prompt: "test input".to_string(),
            system_prompt: None,
            max_tokens: None,
            temperature: None,
            structured_output: None,
        }) {
            Ok(v) => v,
            Err(e) => panic!("echo failed: {e}"),
        };
        assert_eq!(r.content, "test input");
    }

    #[test]
    fn health_check_passes() {
        let p = MockLlmProvider::single("x");
        assert!(p.health().is_ok());
    }

    #[test]
    fn model_info() {
        let p = MockLlmProvider::single("x");
        let info = p.model_info();
        assert_eq!(info.name, "mock");
    }
}
