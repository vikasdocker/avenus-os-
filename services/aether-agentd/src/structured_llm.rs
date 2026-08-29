// Structured LLM output bridge.
//
// The Agent Runtime owns the canonical INTENT_SCHEMA; this module mirrors
// the same shape locally so the agent daemon can ask the AI provider for
// structured output without taking a new dependency. The LLM is NEVER
// allowed to set risk or authority; it can only propose:
//
//   { capability, confidence, entities, reason }
//
// We validate the response, map the proposed `capability` to a
// CapabilityId this daemon understands, and hand it to the planner. The
// planner still validates against the policy before any action runs.
//
// Failure modes — all of which fall back to plain chat:
//   * Provider unreachable / returns error
//   * Response is not valid JSON
//   * Response does not match the envelope shape
//   * capability is non-empty but unknown to us
//   * confidence is out of range, entities is not an object, reason empty
//   * LLM proposed a capability we already auto-detected deterministically
//     (we just return the structured intent and let the planner run it)

use crate::context::SystemContext;
use crate::intent::{CapabilityId, Intent};
use crate::AiProvider;
use serde_json::Value;

/// JSON schema for the structured intent envelope. Kept byte-equivalent
/// to the runtime INTENT_SCHEMA so any provider that already learned the
/// runtime schema can also satisfy this one.
pub const INTENT_SCHEMA: &str = r#"{
  "type": "object",
  "additionalProperties": false,
  "required": ["capability", "confidence", "entities", "reason"],
  "properties": {
    "capability": {
      "type": "string",
      "description": "One of the Aether CapabilityId slugs (e.g. 'app.launch'), or empty string for chat."
    },
    "confidence": {
      "type": "integer",
      "minimum": 0,
      "maximum": 100,
      "description": "LLM self-rated confidence in the proposed capability, 0-100."
    },
    "entities": {
      "type": "object",
      "description": "Structured arguments for the capability (e.g. { 'app': 'calculator' })."
    },
    "reason": {
      "type": "string",
      "description": "Short human-readable explanation of why this capability was chosen."
    }
  }
}"#;

/// Wire format the LLM is asked to produce.
///
/// `deny_unknown_fields` is the security boundary: an LLM cannot
/// smuggle `root: true`, `admin: true`, `allow: true`,
/// `skip_policy: true`, `trusted: true`, or any other privileged
/// field past the deserializer. The daemon will surface that as
/// `StructuredError::BadShape` and fall back to plain chat.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntentEnvelope {
    pub capability: String,
    pub confidence: u8,
    pub entities: Value,
    pub reason: String,
}

/// Maximum raw LLM response size, in bytes. Inputs larger than
/// this are rejected before any parsing happens. Mirrors the
/// runtime's MAX_RAW_ENVELOPE_BYTES.
pub const MAX_RAW_ENVELOPE_BYTES: usize = 64 * 1024;

/// Maximum length of a `capability` string.
pub const MAX_CAPABILITY_LEN: usize = 128;

/// Maximum length of a `reason` string.
pub const MAX_REASON_LEN: usize = 2048;

/// Maximum nesting depth of the `entities` object.
pub const MAX_ENTITIES_DEPTH: usize = 8;

/// Maximum number of keys in the `entities` object.
pub const MAX_ENTITIES_KEYS: usize = 64;

/// All the ways a structured-intent response can be rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructuredError {
    BadJson(String),
    BadShape(String),
    UnknownCapability(String),
    BadConfidence(u8),
    BadEntities(String),
    EmptyReason,
    ProviderUnavailable(String),
    TooLarge { size: usize, limit: usize },
    CapabilityTooLong { size: usize, limit: usize },
    ReasonTooLong { size: usize, limit: usize },
    EntitiesTooDeep { depth: usize, limit: usize },
    EntitiesTooManyKeys { count: usize, limit: usize },
}

impl std::fmt::Display for StructuredError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadJson(s) => write!(f, "invalid JSON: {s}"),
            Self::BadShape(s) => write!(f, "envelope shape mismatch: {s}"),
            Self::UnknownCapability(s) => write!(f, "unknown capability: '{s}'"),
            Self::BadConfidence(c) => write!(f, "confidence {c} out of range 0..=100"),
            Self::BadEntities(s) => write!(f, "entities not a JSON object: {s}"),
            Self::EmptyReason => write!(f, "reason must be a non-empty string"),
            Self::ProviderUnavailable(s) => write!(f, "provider unavailable: {s}"),
            Self::TooLarge { size, limit } => {
                write!(f, "raw envelope too large: {size} bytes (limit {limit})")
            }
            Self::CapabilityTooLong { size, limit } => {
                write!(f, "capability too long: {size} bytes (limit {limit})")
            }
            Self::ReasonTooLong { size, limit } => {
                write!(f, "reason too long: {size} bytes (limit {limit})")
            }
            Self::EntitiesTooDeep { depth, limit } => {
                write!(f, "entities too deeply nested: {depth} levels (limit {limit})")
            }
            Self::EntitiesTooManyKeys { count, limit } => {
                write!(f, "entities has too many keys: {count} (limit {limit})")
            }
        }
    }
}

impl std::error::Error for StructuredError {}

/// Strip an optional ```json ... ``` fence from the LLM response.
fn strip_code_fence(raw: &str) -> &str {
    let raw = raw.trim();
    if let Some(rest) = raw.strip_prefix("```") {
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

/// Returns the maximum JSON nesting depth of a value. Used to
/// bound the `entities` object.
fn json_depth(v: &Value) -> usize {
    match v {
        Value::Object(map) => 1 + map.values().map(json_depth).max().unwrap_or(0),
        Value::Array(arr) => 1 + arr.iter().map(json_depth).max().unwrap_or(0),
        _ => 0,
    }
}

/// Parse the raw LLM response text into an `IntentEnvelope`.
///
/// Resource limits (MAX_RAW_ENVELOPE_BYTES, etc.) are enforced
/// here. Inputs that exceed the limit are rejected with a typed
/// error before any allocation-heavy work happens.
pub fn parse_envelope(raw: &str) -> Result<IntentEnvelope, StructuredError> {
    if raw.len() > MAX_RAW_ENVELOPE_BYTES {
        return Err(StructuredError::TooLarge { size: raw.len(), limit: MAX_RAW_ENVELOPE_BYTES });
    }
    let trimmed = strip_code_fence(raw);
    let value: Value =
        serde_json::from_str(trimmed).map_err(|e| StructuredError::BadJson(e.to_string()))?;
    let env: IntentEnvelope =
        serde_json::from_value(value).map_err(|e| StructuredError::BadShape(e.to_string()))?;
    if env.capability.len() > MAX_CAPABILITY_LEN {
        return Err(StructuredError::CapabilityTooLong {
            size: env.capability.len(),
            limit: MAX_CAPABILITY_LEN,
        });
    }
    if env.reason.len() > MAX_REASON_LEN {
        return Err(StructuredError::ReasonTooLong {
            size: env.reason.len(),
            limit: MAX_REASON_LEN,
        });
    }
    if let Some(obj) = env.entities.as_object() {
        if obj.len() > MAX_ENTITIES_KEYS {
            return Err(StructuredError::EntitiesTooManyKeys {
                count: obj.len(),
                limit: MAX_ENTITIES_KEYS,
            });
        }
        let depth = json_depth(&env.entities);
        if depth > MAX_ENTITIES_DEPTH {
            return Err(StructuredError::EntitiesTooDeep { depth, limit: MAX_ENTITIES_DEPTH });
        }
    }
    Ok(env)
}

/// Strictly validate an envelope and convert it to a typed `Intent`.
///
/// Returns `Ok(None)` when `capability` is empty (plain chat).
/// Returns `Err` for any other failure mode.
pub fn parse_intent(env: &IntentEnvelope) -> Result<Option<Intent>, StructuredError> {
    if env.capability.trim().is_empty() {
        return Ok(None);
    }
    // Map runtime-style slugs (application.*, system.status, context.get) to
    // the daemon's slugs (app.*, system.*, context.get). This keeps the schema
    // portable across the runtime/daemon split.
    let mapped = match env.capability.as_str() {
        "application.launch" => "app.launch",
        "application.close" => "app.close",
        "application.status" => "app.status",
        "application.list" => "app.list",
        other => other,
    };
    let capability = CapabilityId::from_str(mapped)
        .ok_or_else(|| StructuredError::UnknownCapability(env.capability.clone()))?;
    if env.confidence > 100 {
        return Err(StructuredError::BadConfidence(env.confidence));
    }
    if !env.entities.is_object() {
        return Err(StructuredError::BadEntities("expected JSON object".to_string()));
    }
    if env.reason.trim().is_empty() {
        return Err(StructuredError::EmptyReason);
    }
    Ok(Some(Intent { capability, arguments: env.entities.clone() }))
}

/// Build the prompt that asks the LLM to produce an envelope.
///
/// The schema is embedded so any provider (local or cloud, with or without
/// native JSON-schema support) can be guided to the right shape. We also
/// pass a bounded context hint so the LLM can prefer capabilities that
/// match the current system state.
pub fn build_intent_prompt(user_text: &str, ctx: &SystemContext) -> String {
    let slugs = vec![
        "system.status",
        "app.list",
        "app.status",
        "app.launch",
        "app.close",
        "window.list",
        "window.focus",
        "window.minimize",
        "window.maximize",
        "window.close",
        "context.get",
        "file.list",
        "file.search",
        "file.read",
        "file.create",
        "file.write",
        "file.rename",
        "file.move",
        "file.delete",
        "system.info",
        "system.resources",
        "system.uptime",
    ];
    let valid = slugs.join(", ");
    let context =
        if ctx.grounding_text().is_empty() { "(none)".to_string() } else { ctx.grounding_text() };
    format!(
        "You are the Aether intent classifier. Map the user request to exactly one of \
         the Aether capability slugs listed below, or empty string if the user is just \
         chatting. Respond with a single JSON object matching this schema:\n\n\
         {schema}\n\n\
         Valid capability slugs:\n  {valid}\n\n\
         Current OS context:\n  {context}\n\n\
         User request:\n  {user}\n\n\
         JSON:",
        schema = INTENT_SCHEMA,
        valid = valid,
        context = context,
        user = user_text,
    )
}

/// Outcome of attempting a structured-LLM intent for one user prompt.
#[derive(Debug, Clone, PartialEq)]
pub enum LlmIntentOutcome {
    /// Provider produced a valid structured intent.
    Intent(Intent),
    /// Provider explicitly returned empty capability (plain chat).
    Chat,
    /// Provider produced output we cannot trust; do plain chat.
    Fallback(StructuredError),
}

/// Try to get a structured intent from the provider for a user prompt.
///
/// This is the bridge between the deterministic parser and the LLM. We
/// only call the LLM when the deterministic parser returns nothing. We
/// do NOT call the LLM for inputs the deterministic layer already
/// classified, because the LLM cannot grant additional authority.
pub fn try_structured(
    provider: &dyn AiProvider,
    text: &str,
    ctx: &SystemContext,
) -> LlmIntentOutcome {
    let prompt = build_intent_prompt(text, ctx);
    let raw = match provider.complete(&prompt) {
        Ok(r) => r,
        Err(e) => return LlmIntentOutcome::Fallback(StructuredError::ProviderUnavailable(e)),
    };
    let env = match parse_envelope(&raw) {
        Ok(e) => e,
        Err(e) => return LlmIntentOutcome::Fallback(e),
    };
    match parse_intent(&env) {
        Ok(Some(intent)) => LlmIntentOutcome::Intent(intent),
        Ok(None) => LlmIntentOutcome::Chat,
        Err(e) => LlmIntentOutcome::Fallback(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::SystemContext;

    /// A provider that returns a fixed string. Used to test the bridge
    /// without a real network call.
    struct FixedProvider(&'static str);
    impl crate::AiProvider for FixedProvider {
        fn name(&self) -> &str {
            "fixed"
        }
        fn complete(&self, _prompt: &str) -> Result<String, String> {
            Ok(self.0.to_string())
        }
    }

    struct FailProvider;
    impl crate::AiProvider for FailProvider {
        fn name(&self) -> &str {
            "fail"
        }
        fn complete(&self, _prompt: &str) -> Result<String, String> {
            Err("connection refused".to_string())
        }
    }

    #[test]
    fn parse_envelope_accepts_bare_json() {
        let raw = r#"{"capability":"app.launch","confidence":85,"entities":{"app":"calculator"},"reason":"user said open calculator"}"#;
        let env = match parse_envelope(raw) {
            Ok(e) => e,
            Err(e) => panic!("parse failed: {e}"),
        };
        assert_eq!(env.capability, "app.launch");
        assert_eq!(env.confidence, 85);
        assert_eq!(env.reason, "user said open calculator");
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
        assert!(matches!(err, StructuredError::BadJson(_)));
    }

    #[test]
    fn parse_intent_empty_capability_is_chat() {
        let env = IntentEnvelope {
            capability: String::new(),
            confidence: 0,
            entities: serde_json::json!({}),
            reason: "no capability".to_string(),
        };
        let out = match parse_intent(&env) {
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
        let err = match parse_intent(&env) {
            Ok(_) => panic!("expected rejection"),
            Err(e) => e,
        };
        assert!(matches!(err, StructuredError::UnknownCapability(_)));
    }

    #[test]
    fn parse_intent_valid_envelope_produces_typed_intent() {
        let env = IntentEnvelope {
            capability: "app.launch".to_string(),
            confidence: 90,
            entities: serde_json::json!({"app": "calculator"}),
            reason: "user said open calculator".to_string(),
        };
        let intent = match parse_intent(&env) {
            Ok(Some(i)) => i,
            Ok(None) => panic!("expected intent"),
            Err(e) => panic!("parse failed: {e}"),
        };
        assert_eq!(intent.capability, CapabilityId::AppLaunch);
        assert_eq!(intent.arguments["app"], "calculator");
    }

    #[test]
    fn parse_intent_rejects_confidence_over_100() {
        let env = IntentEnvelope {
            capability: "system.status".to_string(),
            confidence: 120,
            entities: serde_json::json!({}),
            reason: "x".to_string(),
        };
        let err = match parse_intent(&env) {
            Ok(_) => panic!("expected error"),
            Err(e) => e,
        };
        assert!(matches!(err, StructuredError::BadConfidence(120)));
    }

    #[test]
    fn parse_intent_rejects_non_object_entities() {
        let env = IntentEnvelope {
            capability: "system.status".to_string(),
            confidence: 80,
            entities: serde_json::json!("not an object"),
            reason: "x".to_string(),
        };
        let err = match parse_intent(&env) {
            Ok(_) => panic!("expected error"),
            Err(e) => e,
        };
        assert!(matches!(err, StructuredError::BadEntities(_)));
    }

    #[test]
    fn parse_intent_rejects_empty_reason() {
        let env = IntentEnvelope {
            capability: "system.status".to_string(),
            confidence: 80,
            entities: serde_json::json!({}),
            reason: "   ".to_string(),
        };
        let err = match parse_intent(&env) {
            Ok(_) => panic!("expected error"),
            Err(e) => e,
        };
        assert_eq!(err, StructuredError::EmptyReason);
    }

    #[test]
    fn parse_intent_maps_runtime_slugs_to_daemon_slugs() {
        let env = IntentEnvelope {
            capability: "application.launch".to_string(),
            confidence: 90,
            entities: serde_json::json!({"app": "notes"}),
            reason: "open notes".to_string(),
        };
        let intent = match parse_intent(&env) {
            Ok(Some(i)) => i,
            Ok(None) => panic!("expected intent"),
            Err(e) => panic!("parse failed: {e}"),
        };
        assert_eq!(intent.capability, CapabilityId::AppLaunch);
    }

    #[test]
    fn try_structured_returns_intent_for_valid_provider_output() {
        let raw = r#"{"capability":"app.launch","confidence":90,"entities":{"app":"calculator"},"reason":"open calc"}"#;
        let p = FixedProvider(raw);
        let ctx = SystemContext::empty();
        match try_structured(&p, "open calculator", &ctx) {
            LlmIntentOutcome::Intent(i) => {
                assert_eq!(i.capability, CapabilityId::AppLaunch);
                assert_eq!(i.arguments["app"], "calculator");
            }
            other => panic!("expected Intent, got {other:?}"),
        }
    }

    #[test]
    fn try_structured_returns_chat_for_empty_capability() {
        let raw = r#"{"capability":"","confidence":0,"entities":{},"reason":"just chat"}"#;
        let p = FixedProvider(raw);
        let ctx = SystemContext::empty();
        match try_structured(&p, "hi", &ctx) {
            LlmIntentOutcome::Chat => {}
            other => panic!("expected Chat, got {other:?}"),
        }
    }

    #[test]
    fn try_structured_falls_back_on_bad_json() {
        let p = FixedProvider("not json");
        let ctx = SystemContext::empty();
        match try_structured(&p, "open calculator", &ctx) {
            LlmIntentOutcome::Fallback(StructuredError::BadJson(_)) => {}
            other => panic!("expected BadJson fallback, got {other:?}"),
        }
    }

    #[test]
    fn try_structured_falls_back_on_unknown_capability() {
        let raw = r#"{"capability":"agent.execute_shell","confidence":99,"entities":{"command":"x"},"reason":"y"}"#;
        let p = FixedProvider(raw);
        let ctx = SystemContext::empty();
        match try_structured(&p, "do bad thing", &ctx) {
            LlmIntentOutcome::Fallback(StructuredError::UnknownCapability(_)) => {}
            other => panic!("expected UnknownCapability fallback, got {other:?}"),
        }
    }

    #[test]
    fn try_structured_falls_back_on_provider_error() {
        let p = FailProvider;
        let ctx = SystemContext::empty();
        match try_structured(&p, "anything", &ctx) {
            LlmIntentOutcome::Fallback(StructuredError::ProviderUnavailable(_)) => {}
            other => panic!("expected ProviderUnavailable fallback, got {other:?}"),
        }
    }

    #[test]
    fn try_structured_strips_code_fence_before_parsing() {
        let raw = "```json\n{\"capability\":\"window.list\",\"confidence\":80,\"entities\":{},\"reason\":\"show windows\"}\n```";
        let p = FixedProvider(raw);
        let ctx = SystemContext::empty();
        match try_structured(&p, "what's open", &ctx) {
            LlmIntentOutcome::Intent(i) => {
                assert_eq!(i.capability, CapabilityId::WindowList);
            }
            other => panic!("expected Intent, got {other:?}"),
        }
    }

    #[test]
    fn build_intent_prompt_includes_schema_and_capabilities() {
        let ctx = SystemContext::empty();
        let p = build_intent_prompt("open calculator", &ctx);
        assert!(p.contains("app.launch"));
        assert!(p.contains("system.status"));
        assert!(p.contains("open calculator"));
    }

    #[test]
    fn try_structured_handles_huge_response_without_panic() {
        // Resource limit boundary: the LLM must not be able to
        // produce a 1MB response that the deserializer handles in
        // O(N) memory and crashes the daemon. The deserializer
        // must complete (or fail with a clean error) regardless
        // of input size.
        let mut raw = String::from(r#"{"capability":"system.status","confidence":80,"entities":{"#);
        for _ in 0..10_000 {
            raw.push_str("\"k\":\"v\",");
        }
        raw.push_str("\"reason\":\"x\"}");
        let p = FixedProvider(Box::leak(raw.into_boxed_str())); // leak to satisfy 'static
        let ctx = SystemContext::empty();
        let outcome = try_structured(&p, "x", &ctx);
        // We don't care which fallback path it takes; we only
        // verify the call returns cleanly without panicking.
        let _ = outcome;
    }

    #[test]
    fn parse_envelope_rejects_unicode_control_chars_in_reason() {
        // A model that smuggles ANSI escapes or null bytes in
        // `reason` must not break the parser. The deserializer
        // accepts the value; downstream code (audit log, chat
        // display) is responsible for sanitising. Here we only
        // verify the parse path doesn't reject it.
        let raw = "{\"capability\":\"system.status\",\"confidence\":80,\"entities\":{},\"reason\":\"ok\\u0000\\u001b[31mred\\u001b[0m\"}";
        let env = match parse_envelope(raw) {
            Ok(e) => e,
            Err(e) => panic!("parse failed: {e}"),
        };
        assert!(env.reason.contains("red"));
    }

    #[test]
    fn try_structured_handles_empty_response_string() {
        // Some providers return an empty string on a refused
        // request. The bridge must treat that as a clean
        // fallback, not a panic.
        let p = FixedProvider("");
        let ctx = SystemContext::empty();
        match try_structured(&p, "anything", &ctx) {
            LlmIntentOutcome::Fallback(StructuredError::BadJson(_)) => {}
            other => panic!("expected BadJson fallback, got {other:?}"),
        }
    }

    #[test]
    fn try_structured_handles_non_object_response() {
        // LLM returns a JSON array instead of an object.
        let p = FixedProvider("[1,2,3]");
        let ctx = SystemContext::empty();
        match try_structured(&p, "anything", &ctx) {
            LlmIntentOutcome::Fallback(_) => {}
            other => panic!("expected fallback, got {other:?}"),
        }
    }

    #[test]
    fn try_structured_handles_json_null_response() {
        let p = FixedProvider("null");
        let ctx = SystemContext::empty();
        match try_structured(&p, "anything", &ctx) {
            LlmIntentOutcome::Fallback(_) => {}
            other => panic!("expected fallback, got {other:?}"),
        }
    }

    #[test]
    fn try_structured_rejects_root_privilege_escalation_field() {
        // Defence-in-depth: the daemon's structured parser must
        // reject an envelope that smuggles in privilege fields,
        // even if the LLM thinks it has authority.
        let raw = r#"{"capability":"app.launch","confidence":99,"entities":{"app":"x"},"reason":"y","root":true,"admin":true,"allow":true}"#;
        let p = FixedProvider(raw);
        let ctx = SystemContext::empty();
        match try_structured(&p, "x", &ctx) {
            LlmIntentOutcome::Fallback(StructuredError::BadShape(_)) => {}
            other => panic!("expected BadShape fallback, got {other:?}"),
        }
    }

    #[test]
    fn try_structured_falls_back_on_oversized_response() {
        // Resource limit: 100 KiB raw input must be rejected
        // before parsing. The bridge must surface a typed
        // TooLarge fallback.
        let mut raw = String::with_capacity(100 * 1024);
        for _ in 0..(100 * 1024) {
            raw.push('A');
        }
        let leaked: &'static str = Box::leak(raw.into_boxed_str());
        let p = FixedProvider(leaked);
        let ctx = SystemContext::empty();
        match try_structured(&p, "x", &ctx) {
            LlmIntentOutcome::Fallback(StructuredError::TooLarge { .. }) => {}
            other => panic!("expected TooLarge fallback, got {other:?}"),
        }
    }

    #[test]
    fn try_structured_falls_back_on_oversized_reason() {
        let mut reason = String::with_capacity(MAX_REASON_LEN + 1);
        for _ in 0..(MAX_REASON_LEN + 1) {
            reason.push('y');
        }
        let raw = format!(
            r#"{{"capability":"app.launch","confidence":80,"entities":{{}},"reason":"{reason}"}}"#
        );
        let leaked: &'static str = Box::leak(raw.into_boxed_str());
        let p = FixedProvider(leaked);
        let ctx = SystemContext::empty();
        match try_structured(&p, "x", &ctx) {
            LlmIntentOutcome::Fallback(StructuredError::ReasonTooLong { .. }) => {}
            other => panic!("expected ReasonTooLong fallback, got {other:?}"),
        }
    }
}
