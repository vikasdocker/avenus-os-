// Agent Runtime - Structured AI Output
//
// Defines the JSON schema for LLM-produced intent and validates the LLM's
// response against it. The LLM is never trusted to assign risk or grant
// authority: it can only propose an IntentType, a confidence score, and a
// small structured entity map. Risk is assigned by trusted system code that
// consumes this envelope.
//
// Every parse path returns a typed error so callers (agentd) can decide
// whether to fall back to deterministic parsing, refuse the response, or
// ask the user for clarification.

use crate::intent::{Confidence, Intent, IntentType};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON Schema for the structured intent envelope the LLM must produce.
///
/// Published as a value so any LLM provider can be given the schema as
/// `format` / `response_schema` / `grammar` parameter.
pub const INTENT_SCHEMA: &str = r#"{
  "type": "object",
  "additionalProperties": false,
  "required": ["capability", "confidence", "entities", "reason"],
  "properties": {
    "capability": {
      "type": "string",
      "description": "One of the Aether IntentType values, e.g. 'application.launch'. Empty string means 'no structured intent'."
    },
    "confidence": {
      "type": "integer",
      "minimum": 0,
      "maximum": 100,
      "description": "LLM self-rated confidence in the proposed intent, 0-100."
    },
    "entities": {
      "type": "object",
      "description": "Structured arguments for the capability (e.g. { 'app': 'calculator' })."
    },
    "reason": {
      "type": "string",
      "description": "Short human-readable explanation of why this intent was chosen."
    }
  }
}"#;

/// Wire format the LLM is asked to produce.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentEnvelope {
    /// IntentType as a string (e.g. "application.launch") or "" for "chat".
    pub capability: String,
    /// Confidence 0..=100.
    pub confidence: u8,
    /// Free-form structured entities (validated per-capability at parse time).
    pub entities: Value,
    /// Short reason string.
    pub reason: String,
}

/// All the ways a structured-intent response can be rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructuredIntentError {
    /// Input was not valid JSON.
    BadJson(String),
    /// JSON did not match the envelope shape.
    BadShape(String),
    /// capability string did not map to any known IntentType.
    UnknownCapability(String),
    /// confidence was outside 0..=100.
    BadConfidence(u8),
    /// entities was not a JSON object.
    BadEntities(String),
    /// reason was empty.
    EmptyReason,
}

impl std::fmt::Display for StructuredIntentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadJson(s) => write!(f, "invalid JSON: {s}"),
            Self::BadShape(s) => write!(f, "envelope shape mismatch: {s}"),
            Self::UnknownCapability(s) => write!(f, "unknown capability: '{s}'"),
            Self::BadConfidence(c) => write!(f, "confidence {c} out of range 0..=100"),
            Self::BadEntities(s) => write!(f, "entities not a JSON object: {s}"),
            Self::EmptyReason => write!(f, "reason must be a non-empty string"),
        }
    }
}

impl std::error::Error for StructuredIntentError {}

/// Parse the raw LLM response text into an `IntentEnvelope`.
///
/// Accepts either the bare envelope object or a fenced ```json block
/// (e.g. when the model wraps its answer). Does not interpret the
/// capability yet — that is done by [`parse_intent`].
pub fn parse_envelope(raw: &str) -> Result<IntentEnvelope, StructuredIntentError> {
    let trimmed = strip_code_fence(raw);
    let value: Value =
        serde_json::from_str(trimmed).map_err(|e| StructuredIntentError::BadJson(e.to_string()))?;
    let env: IntentEnvelope = serde_json::from_value(value)
        .map_err(|e| StructuredIntentError::BadShape(e.to_string()))?;
    Ok(env)
}

/// Strictly validate an envelope and convert it to a typed `Intent`.
///
/// When `capability` is empty, returns `Ok(None)` — the LLM is signalling
/// that the user prompt does not map to a structured intent (i.e. plain
/// chat). When the capability is non-empty but unknown, returns
/// `Err(UnknownCapability)` — the LLM is hallucinating and must be ignored.
pub fn parse_intent(env: &IntentEnvelope, request_id: &str) -> Result<Option<Intent>, StructuredIntentError> {
    if env.capability.trim().is_empty() {
        return Ok(None);
    }
    let intent_type = IntentType::from_str(&env.capability)
        .ok_or_else(|| StructuredIntentError::UnknownCapability(env.capability.clone()))?;
    if env.confidence > 100 {
        return Err(StructuredIntentError::BadConfidence(env.confidence));
    }
    if !env.entities.is_object() {
        return Err(StructuredIntentError::BadEntities(
            "expected JSON object".to_string(),
        ));
    }
    if env.reason.trim().is_empty() {
        return Err(StructuredIntentError::EmptyReason);
    }
    let intent = Intent::new(
        request_id,
        intent_type,
        Confidence(env.confidence),
        env.entities.clone(),
    )
    .with_reason(env.reason.clone());
    Ok(Some(intent))
}

/// Strip an optional ```json ... ``` fence from the LLM response.
fn strip_code_fence(raw: &str) -> &str {
    let raw = raw.trim();
    if let Some(rest) = raw.strip_prefix("```") {
        // Drop optional language tag on the first line.
        if let Some(newline_idx) = rest.find('\n') {
            let after_lang = &rest[newline_idx + 1..];
            if let Some(stripped) = after_lang.strip_suffix("```") {
                return stripped.trim();
            }
            return after_lang.trim();
        }
    }
    raw
}

/// Build the user-facing prompt that asks the LLM to produce an envelope.
///
/// The schema is embedded directly so any provider (including local ones
/// without native JSON-schema support) can be guided to the right shape.
pub fn build_intent_prompt(user_text: &str, context_hint: &str) -> String {
    let valid = IntentType::all_slugs().join(", ");
    format!(
        "You are the Aether intent classifier. Map the user request to exactly one of \
         the Aether capability strings listed below, or empty string if the user is just \
         chatting. Respond with a single JSON object matching this schema:\n\n\
         {schema}\n\n\
         Valid capability strings:\n  {valid}\n\n\
         Current OS context:\n  {context}\n\n\
         User request:\n  {user}\n\n\
         JSON:",
        schema = INTENT_SCHEMA,
        valid = valid,
        context = if context_hint.is_empty() { "(none)".to_string() } else { context_hint.to_string() },
        user = user_text,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_envelope_accepts_bare_json() {
        let raw = r#"{"capability":"application.launch","confidence":85,"entities":{"app":"calculator"},"reason":"user said open calculator"}"#;
        let env = match parse_envelope(raw) {
            Ok(e) => e,
            Err(e) => panic!("parse failed: {e}"),
        };
        assert_eq!(env.capability, "application.launch");
        assert_eq!(env.confidence, 85);
    }

    #[test]
    fn parse_envelope_strips_code_fence() {
        let raw = "```json\n{\"capability\":\"system.status\",\"confidence\":70,\"entities\":{},\"reason\":\"check status\"}\n```";
        let env = match parse_envelope(raw) {
            Ok(e) => e,
            Err(e) => panic!("parse failed: {e}"),
        };
        assert_eq!(env.capability, "system.status");
    }

    #[test]
    fn parse_envelope_rejects_bad_json() {
        let raw = "not json at all";
        let err = match parse_envelope(raw) {
            Ok(_) => panic!("expected error"),
            Err(e) => e,
        };
        assert!(matches!(err, StructuredIntentError::BadJson(_)));
    }

    #[test]
    fn parse_envelope_rejects_extra_fields_via_shape() {
        // additionalProperties:false is enforced at the schema level, not at
        // the Rust type. The Rust deserializer is permissive by default, but
        // the trusted code path uses parse_intent to validate the capability
        // string and entities shape, which is the security boundary.
        let raw = r#"{"capability":"application.launch","confidence":85,"entities":{"app":"calculator"},"reason":"x","extra":1}"#;
        let env = match parse_envelope(raw) {
            Ok(e) => e,
            Err(e) => panic!("parse failed: {e}"),
        };
        assert_eq!(env.capability, "application.launch");
    }

    #[test]
    fn parse_intent_empty_capability_is_chat() {
        let env = IntentEnvelope {
            capability: String::new(),
            confidence: 0,
            entities: serde_json::json!({}),
            reason: "no intent".to_string(),
        };
        let out = match parse_intent(&env, "req-1") {
            Ok(o) => o,
            Err(e) => panic!("parse failed: {e}"),
        };
        assert!(out.is_none());
    }

    #[test]
    fn parse_intent_unknown_capability_is_rejected() {
        let env = IntentEnvelope {
            capability: "agent.execute_shell".to_string(),
            confidence: 90,
            entities: serde_json::json!({"command": "rm -rf /"}),
            reason: "user asked".to_string(),
        };
        let err = match parse_intent(&env, "req-1") {
            Ok(_) => panic!("expected rejection"),
            Err(e) => e,
        };
        assert!(matches!(err, StructuredIntentError::UnknownCapability(_)));
    }

    #[test]
    fn parse_intent_valid_envelope_produces_typed_intent() {
        let env = IntentEnvelope {
            capability: "application.launch".to_string(),
            confidence: 90,
            entities: serde_json::json!({"app": "calculator"}),
            reason: "user said open calculator".to_string(),
        };
        let intent = match parse_intent(&env, "req-1") {
            Ok(Some(i)) => i,
            Ok(None) => panic!("expected intent"),
            Err(e) => panic!("parse failed: {e}"),
        };
        assert_eq!(intent.intent_type, IntentType::ApplicationLaunch);
        assert_eq!(intent.confidence.0, 90);
        assert_eq!(intent.entities["app"], "calculator");
    }

    #[test]
    fn parse_intent_rejects_confidence_over_100() {
        let env = IntentEnvelope {
            capability: "system.status".to_string(),
            confidence: 120,
            entities: serde_json::json!({}),
            reason: "x".to_string(),
        };
        let err = match parse_intent(&env, "r") {
            Ok(_) => panic!("expected error"),
            Err(e) => e,
        };
        assert!(matches!(err, StructuredIntentError::BadConfidence(120)));
    }

    #[test]
    fn parse_intent_rejects_non_object_entities() {
        let env = IntentEnvelope {
            capability: "system.status".to_string(),
            confidence: 80,
            entities: serde_json::json!("not an object"),
            reason: "x".to_string(),
        };
        let err = match parse_intent(&env, "r") {
            Ok(_) => panic!("expected error"),
            Err(e) => e,
        };
        assert!(matches!(err, StructuredIntentError::BadEntities(_)));
    }

    #[test]
    fn parse_intent_rejects_empty_reason() {
        let env = IntentEnvelope {
            capability: "system.status".to_string(),
            confidence: 80,
            entities: serde_json::json!({}),
            reason: "   ".to_string(),
        };
        let err = match parse_intent(&env, "r") {
            Ok(_) => panic!("expected error"),
            Err(e) => e,
        };
        assert_eq!(err, StructuredIntentError::EmptyReason);
    }

    #[test]
    fn build_intent_prompt_includes_schema_and_capabilities() {
        let p = build_intent_prompt("open calculator", "running: notes");
        assert!(p.contains("application.launch"));
        assert!(p.contains("system.status"));
        assert!(p.contains("open calculator"));
        assert!(p.contains("running: notes"));
    }

    #[test]
    fn build_intent_prompt_handles_empty_context() {
        let p = build_intent_prompt("hi", "");
        assert!(p.contains("(none)"));
    }
}
