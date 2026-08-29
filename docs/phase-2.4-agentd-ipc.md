# aether-agentd IPC Reference

`aether-agentd` listens on port 4748 (default; override with
`AETHER_AGENT_PORT`) for newline-delimited JSON requests.

## Wire format

Request:

```json
{"command": "agent.intent", "argument": "{\"session_id\":\"...\",\"capability\":\"app.launch\",\"arguments\":{\"app\":\"calculator\"}}"}
```

Response:

```json
{"ok": true, "result": { ... }}
{"ok": false, "result": {"error": "..."}}
```

## Commands

| Command | Argument | Returns |
| --- | --- | --- |
| `agent.status` | — | `{agent_id, health, event_count, task_count, provider}` |
| `agent.session.create` | user identity string | `{session_id, actor, status}` |
| `agent.session.list` | — | `{sessions: [...]}` |
| `agent.session.status` | session UUID | `{session: {...}}` |
| `agent.session.cancel` | session UUID | `{cancelled: bool, note?}` |
| `agent.intent` | JSON envelope (see below) | `{response, actions, provider}` |
| `agent.audit.recent` | count (default 20) | `{entries: [...]}` |
| `agent.audit.session` | session UUID | `{entries: [...]}` |
| `agent.action.cancel` | action UUID | `{cancelled: bool}` |
| `agent.stop` | — | `{stopped: true}` |

### Intent envelope

`agent.intent` accepts a JSON envelope of the form:

```json
{
  "session_id": "<uuid>",
  "capability": "app.launch",
  "arguments": { "app": "calculator" }
}
```

or a freeform `IntentEnvelope` produced by an LLM:

```json
{
  "capability": "system.status",
  "confidence": 92,
  "entities": {},
  "reason": "user asked for system health"
}
```

The daemon runs the envelope through the structured-intent parser
and the `intent_to_action` mapper. Unknown capabilities return
`{ok: false, error: "unknown capability"}`.

## Example session

```bash
# 1) Create a session
echo '{"command":"agent.session.create","argument":"alice"}' | nc 127.0.0.1 4748
# -> {"ok":true,"result":{"session_id":"...","actor":"alice","status":"Ready"}}

# 2) Submit an intent
SID=...
echo "{\"command\":\"agent.intent\",\"argument\":\"{\\\"session_id\\\":\\\"$SID\\\",\\\"capability\\\":\\\"app.launch\\\",\\\"arguments\\\":{\\\"app\\\":\\\"calculator\\\"}}\"}" | nc 127.0.0.1 4748

# 3) Inspect the session
echo "{\"command\":\"agent.session.status\",\"argument\":\"$SID\"}" | nc 127.0.0.1 4748

# 4) Cancel
echo "{\"command\":\"agent.session.cancel\",\"argument\":\"$SID\"}" | nc 127.0.0.1 4748
```

## Error codes

| Code | Meaning |
| --- | --- |
| `BAD_REQUEST` | Malformed JSON or missing fields |
| `UNKNOWN_CAPABILITY` | Capability is not in the trusted table |
| `UNKNOWN_SESSION` | Session ID does not exist |
| `ALREADY_TERMINAL` | Replay attempt on a terminal session |
| `POLICY_DENIED` | Action requires a capability the actor does not have |
| `INTERNAL` | Unhandled error in the runtime or executor |
