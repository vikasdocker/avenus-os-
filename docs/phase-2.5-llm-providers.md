# Phase 2.5 — LLM Provider Layer

This document describes the runtime LLM provider layer. The trait and
the mocks existed before; this phase ships the first two non-mock
providers and the selection function.

## What ships

- `LlmProvider` trait (in `aether-agent-runtime/src/llm.rs`):
  `generate`, `stream`, `structured_output`, `health`, `model_info`.
- `MockLlmProvider`, `EchoLlmProvider` (in `llm.rs`).
- `OllamaLlmProvider` (in `llm_provider.rs`): HTTP/1.1 POST to
  `<url>/api/chat` with the messages-array shape Ollama expects.
  Supports `format` for structured output. Includes a `health()`
  probe via `<url>/api/tags`.
- `OpenAILlmProvider` (in `llm_provider.rs`): HTTP/1.1 POST to
  `<url>/v1/chat/completions`. Supports Bearer auth, `max_tokens`,
  `temperature`, and `response_format` for structured output. Includes
  a `health()` probe via `<url>/v1/models`.
- `select(provider, url, model, api_key) -> Box<dyn LlmProvider>`:
  pure function. Kinds: `echo`, `mock`, `ollama`, `openai`. Unknown
  kinds fall back to `echo`.
- `select_from_env()`: production wrapper that reads
  `AETHER_LLM_PROVIDER`, `AETHER_LLM_URL`, `AETHER_LLM_MODEL`,
  `AETHER_LLM_API_KEY` and calls `select`.
- `RuntimeBackedProvider` (in `aether-agentd`): adapter from the
  runtime's `LlmProvider` to the daemon's flat `AiProvider` interface
  so the daemon and the runtime speak the exact same HTTP shape.

## Wire formats

### Ollama (`/api/chat`)

Request:
```json
{
  "model": "llama3.2",
  "messages": [
    {"role": "system", "content": "be brief"},
    {"role": "user", "content": "hi"}
  ],
  "stream": false,
  "format": <schema>            // optional, structured output
}
```

Response (collapsed from NDJSON if needed):
```json
{
  "model": "llama3.2",
  "message": {"role": "assistant", "content": "hi"},
  "done": true
}
```

### OpenAI-compatible (`/v1/chat/completions`)

Request:
```json
{
  "model": "gpt-3.5-turbo",
  "messages": [...],
  "max_tokens": 256,
  "temperature": 0.7,
  "response_format": {"type": "json_schema", "schema": <schema>}
}
```

Response:
```json
{
  "model": "gpt-3.5-turbo",
  "choices": [
    {
      "index": 0,
      "message": {"role": "assistant", "content": "hi"},
      "finish_reason": "stop"
    }
  ],
  "usage": {"total_tokens": 7}
}
```

## Selection env vars

| Variable | Purpose | Default |
| --- | --- | --- |
| `AETHER_LLM_PROVIDER` | Kind: `echo` \| `mock` \| `ollama` \| `openai` | `echo` |
| `AETHER_LLM_URL` | Provider-specific base URL | kind-specific |
| `AETHER_LLM_MODEL` | Model name | kind-specific |
| `AETHER_LLM_API_KEY` | Bearer token (OpenAI only) | unset |

The agentd has its own selection function
`provider_from_selection` for backward compatibility that supports the
older `AETHER_AI_PROVIDER` env. The new `runtime-ollama` kind
selects the runtime-backed bridge.

## Failure modes

Every error path is a structured `Err(String)` with a clear message.
The runtime never silently falls back to a mock — the daemon's
structured-intent parser is responsible for distinguishing real
model output from a deterministic echo.

Tested:

- 200 OK happy path (Ollama, OpenAI).
- 4xx / 5xx error surfaces as `http 4xx / 5xx: <body-snippet>`.
- Missing fields in response surface as
  `missing message.content` / `missing choices[0].message.content`.
- Connection failure surfaces as `connect <authority>: <error>`.
- Auth header is sent when an API key is set.
- Selection falls back to `echo` for unknown kinds.

## Known limitations

- Streaming responses are not yet supported; the `stream()` method
  defaults to a blocking `generate()` call. A future phase will add
  real SSE / NDJSON streaming.
- The OpenAI adapter does not yet handle function-calling or
  tool-calling responses.
- The runtime does not yet rate-limit the LLM calls or apply
  per-session quotas. The agentd's request handler should enforce
  this at the IPC boundary.
- Token counting is reported by OpenAI but not by Ollama (it is
  `None` for the Ollama path).
