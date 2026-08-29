# Phase 2.4 — Agent ↔ OS Integration

This document describes how the Aether Agent Runtime is embedded into
the Aether OS control plane. It complements `aether-os-architecture.md`
with a concrete walkthrough of every layer and every security
boundary.

## 1. Agent Runtime

The runtime is the trusted code that owns the LLM's output. The LLM
itself is **never** the OS. Anything the LLM proposes is treated as
untrusted input and validated by code that is not produced by the
LLM.

**Where it lives:** `agent/aether-agent-runtime/src/`

Key modules:

- `host.rs` — `AgentRuntimeHost` lifecycle state machine.
- `session.rs` — `AgentSession`, `SessionId`, `SessionState`.
- `intent.rs` — `Intent`, `IntentType`, `Confidence`.
- `structured_intent.rs` — canonical `INTENT_SCHEMA`, `parse_intent`,
  `parse_envelope`, `build_intent_prompt`. The LLM is required to
  return an `IntentEnvelope` JSON object; anything else is rejected.
- `action.rs` — `Action`, `ActionId`, `ActionVariant`. Every variant
  is a typed, auditable, capability-tagged unit of work. Variants
  include `ApplicationLaunch`, `ApplicationClose`, `WindowList`,
  `WindowFocus`, `FileList`, `FileRead`, `FileCreate`, `FileWrite`,
  `FileSearch`, `FileRename`, `FileMove`, `FileDelete`,
  `ProcessList`, `ProcessInspect`, `NetworkStatus`,
  `NetworkInterfaces`, `SystemStatus`, `SystemInfo`,
  `SystemResources`, `SystemUptime`, `StorageStatus`, `ContextGet`.
- `planner.rs` — turns a validated intent into a `Plan` of typed
  actions. Risk classification is **trusted** (`classify_action` is
  not exposed to the LLM).
- `validator.rs` — semantic validation of arguments (paths, app IDs,
  etc.) before capability gating.
- `executor.rs` — runs the plan, talks to Aether IPC, produces
  observations.
- `observation.rs` — structured observations only; raw text never
  flows back to the LLM unfiltered.
- `audit.rs` — append-only audit ring with session/request/intent/
  plan/action/capability/policy/execution/result/failure/cancellation
  event types.
- `events.rs` — typed `AetherAgentEvent`s published on the event bus.
- `cancellation.rs` — `CancellationToken` for any in-flight action.
- `recovery.rs` — `decide_recovery` + `RecoveryPolicy`. Read-only
  capabilities get a transient-default retry; mutating capabilities
  get exactly one retry.
- `errors.rs` — `AgentError` with explicit kinds.
- `tool.rs` — `ToolRegistry` of capability tags.
- `approval.rs` — `ApprovalRequest`, `ApprovalDecision`,
  `ApprovalStatus`. Used when an action needs explicit user consent.
- `memory.rs` — per-session `SessionMemory`, multi-session
  `ConversationMemory`.
- `llm.rs` — `LlmProvider` trait and request/response types.
- `request.rs` — `UserRequest`, `RequestActor`.

## 2. agentd Integration

**Where it lives:** `services/aether-agentd/src/`

`aether-agentd` is a long-running daemon. On startup it binds:

- Control plane: port 4747 (speak to `aether-system-core`)
- Surface plane: port 4750 (speak to `aether-graphical-shell`)
- Agent plane: port 4748 (speak to the Agent Runtime)

The daemon's `runtime_host.rs` constructs an `AgentRuntimeHost`
backed by an `InMemoryEventBus` and the daemon's own
`AetherIpcClient` (the bridge to system services).

**Identity** is generated on host construction:

```rust
let host = AgentRuntimeHost::new(HostId::new(), Box::new(bus), Arc::new(ipc));
```

`HostId` is a UUIDv4, stable for the life of the host. The daemon
exposes it through `agent.status`.

## 3. Identity

Every interaction is tagged with:

- **Agent ID** — the host, stable for the daemon process.
- **Session ID** — created on `agent.session.create`, scoped to one
  user task.
- **Request ID** — per `agent.intent` submission.
- **Action ID** — per individual typed `Action`.

These IDs flow into every audit entry, every event, and every
observation. The intent_to_action mapper preserves the chain so an
outside auditor can follow an intent from the LLM envelope to the
specific IPC call that reached a privileged service.

## 4. IPC

**Library ↔ daemon:** NDJSON over TCP loopback (port 4748). The
shell (`aether-shell`) speaks the same wire format via
`shell/aether-shell/src/agentd_client.rs`.

**Daemon ↔ system:** Typed JSON-RPC over loopback to
`aether-system-core` (control plane) and `aether-graphical-shell`
(surface plane). Commands are namespaced:

- `system.*` — health, info, resources, uptime, services.
- `app.*` — list, launch, close, inspect, status.
- `fs.*` — list, stat, search, read, write, create, rename, move,
  delete, mounts, storage.
- `process.*` — list, inspect, start, stop, restart.
- `window.*` — list, focus, minimize, maximize, close, inspect.
- `network.*` — status, interfaces, addresses, routes, dns,
  connectivity, stats.
- `agent.*` — status, session.create, session.list, session.status,
  session.cancel, intent, audit.recent, audit.session,
  action.cancel, stop.

Every IPC call is authenticated by the source service identity
recorded in the manifest (`security_identity`).

## 5. Capabilities

Capabilities are short string identifiers (e.g. `app.launch`,
`system.control.shutdown`, `file.delete`). They are declared by
the runtime, enforced by the daemon, and audited at every gate.

Mapping (capability → ActionVariant) is hard-coded in
`services/aether-agentd/src/intent_to_action.rs`. Adding a new
capability requires editing that file and adding a test — there is
no LLM-driven path that introduces a new capability.

**Shell capabilities are rejected at the type boundary.** Even an
LLM that wants to invoke `agent.execute_shell`, `system.exec`, or
`shell.exec` will be denied because those strings never appear in
the capability table. The malicious-provider security test
(`security_provider_cannot_propose_shell_capability`) proves this.

## 6. Policy

The policy layer is split into:

- **Risk classification** (`ActionRisk::Low | Medium | High |
  Critical`) — set by `classify_action` in the runtime, never by
  the LLM.
- **Confirmation gate** — `High` and `Critical` actions require
  `ApprovalDecision::Confirmed` before the executor runs.
- **Rate limits / quotas** — applied at the daemon before forwarding
  to the runtime.

Policy violations (e.g. attempting `system.control.shutdown`
without the corresponding capability) return an `AgentError` with
kind `Policy` and an audit entry of type `policy.denied`.

## 7. Action Execution

Each `Action` is executed by `ActionExecutor`:

1. **Pre-check** — argument validation, capability lookup, risk
   classification. Failures short-circuit with a clean
   `ExecutionResult::Failed`.
2. **IPC dispatch** — the executor calls the right `aether-system-core`
   command (e.g. `app.launch` for `ActionVariant::ApplicationLaunch`).
   The IPC layer has its own timeout and reconnection policy.
3. **Observation** — the IPC response is normalised into an
   `Observation`. The LLM only ever sees the observation, never the
   raw service reply.
4. **Audit** — `action.requested`, `action.executing`,
   `action.completed` / `action.failed` are appended to the audit
   ring and emitted on the event bus.
5. **Recovery** — transient failures (e.g. network blip) are retried
   per the policy. Permanent failures (e.g. capability denied) are
   not retried.

## 8. Observations

Observations are typed (`ObservationType`) and carry only the data
the LLM is allowed to see. They never include:

- Raw shell output.
- Protected file contents (e.g. `/etc/shadow`).
- Other users' session data.
- Internal stack traces or panic messages.

The `security_rejections_for_file_access` test verifies the
observation does not leak protected content even when the chat
prompt asks for it.

## 9. Event Bus

`InMemoryEventBus` is a bounded ring of typed `AetherAgentEvent`s.
Subscribers (audit consumer, status UI, monitor) get a fresh
snapshot via `recent_events(n)`.

The bus is `Send + Sync` and is the only means of cross-task
notification inside the agent. It is **not** the IPC to the rest
of the OS — that goes through `AetherIpcClient`.

## 10. Security

The full attack surface is covered by 12 security tests:

| Test | Class |
| --- | --- |
| `security_prompt_injection_cannot_invoke_shell` | Prompt injection |
| `security_provider_cannot_propose_shell_capability` | Provider hostility |
| `security_malformed_intent_payload_returns_clean_error` | Malformed payload |
| `security_invalid_capability_is_rejected` | Invalid capability |
| `security_cross_session_lookup_rejects_unknown` | Cross-session |
| `security_replay_cancel_twice_fails_cleanly` | Replay |
| `security_invalid_service_returns_connect_error` | Invalid service |
| `security_privilege_escalation_rejects_high_risk_action` | Privilege escalation |
| `security_malicious_tool_output_is_treated_as_data` | Malicious tool output |
| `security_unauthorized_actions_fail_without_session` | Unauthorized |
| `security_policy_denies_high_risk_capability` | Policy denial |
| `security_empty_chat_does_not_invoke_provider` | Empty input |

**Hard rules enforced everywhere:**

- No `agent.execute_shell`, no `shell.exec`, no `system()` or
  `popen()` for Agent-controlled operations.
- The LLM never assigns its own capability, risk, or authority.
- Every privileged action is structured, authorized, audited,
  bounded, and cancellable.
- Capabilities, permissions, policies, IPC authorization, audit,
  and isolation are **never** weakened to make a test pass.

## 11. End-to-End Flow

The single end-to-end test
`e2e_open_test_application_through_runtime` does:

1. Spawn a loopback mock of `aether-system-core` on a random port.
2. Build an `AgentState` pointed at the mock.
3. `agent.session.create` — captures a real session ID.
4. `agent.intent` — submits a `app.launch / calculator` envelope.
5. The runtime plans one `ActionVariant::ApplicationLaunch`.
6. The executor sends `app.launch` over TCP to the mock control
   plane.
7. The mock reports success; the executor records an
   `Observation::ApplicationLaunched { application_id: "calculator" }`.
8. The audit ring contains `session.created`,
   `action.requested`, `action.completed`, `session.completed`.

The test asserts each of those steps against the daemon's response.

## 12. Known Limitations

- **Real LLM provider** — the production Ollama adapter exists
  (`OllamaProvider`) and is env-gated, but tests use
  `DeterministicIntentProvider` and `MaliciousMockProvider`. End-
  to-end Ollama validation requires a running Ollama instance.
- **No active connectivity probing.** `network.connectivity` is
  derived from interface state and default-route presence.
- **No netlink subscription.** Link-state events are not yet
  pushed onto the bus; a future phase will add a real subscriber.
- **No persistent audit storage.** The audit ring is in-memory and
  bounded. A follow-up phase will write to disk via a privileged
  service.
- **QEMU validation script requires QEMU on a Linux host.** The
  build host for this phase is Windows; QEMU validation runs in
  the QEMU-on-Linux CI / on the maintainer's box. The script and
  the prior smoke logs (`build/qemu-smoke.log`) are the
  reproducible artefact.
- **No `network.apply` or other mutating network commands.** The
  network crate is read-only. Future phases add them with
  capability gating.
