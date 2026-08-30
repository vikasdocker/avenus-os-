// Aether Store — install, launch, and uninstall of signed Aether apps
// (Phase 9.4).
//
// The Store is the *user-facing* application platform. It binds the
// three prior Phase 9 deliverables into a single user-visible flow:
//
//   * Phase 9.2 (aether_core::app)            — typed AppPackage format
//   * Phase 9.2 (aether_security::app_signing)— Ed25519 signature
//   * Phase 9.3 (aether_security::app_security) — consent record,
//                                               install-time audit,
//                                               runtime permission gate
//
// Design rules:
//
//   1. The Store is *pure with respect to I/O*. Every filesystem read
//      and write goes through the `StoreFs` trait, which has two
//      production implementations (`LocalFs` for the real filesystem,
//      and an in-memory `MemoryFs` for tests). This keeps the install
//      and launch paths testable on Windows and on CI.
//
//   2. The Store does NOT spawn apps itself. The actual exec is
//      delegated to `aether-sandbox` via the `Launcher` trait, which
//      in production invokes the Phase 11.4 enforcement binary. The
//      `aether-application-manager` crate remains the OS-level
//      lifecycle owner for legacy built-in apps; the Store is the
//      equivalent surface for *user-installed* apps.
//
//   3. Every privileged action is recorded in the audit log. The
//      Store holds its own `AuditChain`; the system-core's chain is a
//      different (larger) chain. Two chains is intentional: the
//      store's chain captures the user-facing install/launch history
//      without polluting the system-core chain with per-app events.
//
//   4. The trusted-publisher registry is on disk in JSON
//      (`trust.json`); the Store reads it once at construction. The
//      `verify_with_key` path is exposed for tests so unit tests
//      don't have to round-trip through a JSON file.
//
// Phase boundaries:
//   * `aether_security::app_signing` owns signature verification.
//   * `aether_security::app_security` owns consent + install-time audit.
//   * `aether_security::app_security::sandbox_plan_for_app` owns the
//     sandbox plan derivation.
//   * `aether_store` owns the user-facing install/launch/uninstall
//     state machine, the trust registry, the persistence layer, and
//     the audit chain that captures the user-facing history.

pub mod fs;
pub mod registry;
pub mod store;

pub use fs::{LocalFs, MemoryFs, StoreFs};
pub use registry::{PublisherTrust, TrustedPublisherRegistry};
pub use store::{
    AppInstallRecord, InstalledApp, LaunchOutcome, Store, StoreError, StoreResult,
    UntrustedReason,
};
