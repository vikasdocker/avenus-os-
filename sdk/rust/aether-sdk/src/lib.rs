// Aether SDK - Rust client library for the Aether OS control plane.
//
// The SDK has two halves:
//
//   1. A runtime client (`AetherClient`) that speaks the
//      newline-delimited JSON IPC protocol (IpcRequest /
//      IpcResponse from aether-core) over TCP loopback,
//      matching the protocol served by `aether-system-core`.
//      App authors use this to install, launch, and
//      uninstall their package from a CI pipeline or a
//      developer-mode CLI.
//
//   2. A packaging surface (`AppManifestBuilder` and
//      `AppPackageBuilder`) for third-party Aether apps.
//      The manifest builder turns a minimal set of inputs
//      (app id, name, version, publisher, payload) into
//      a valid `AppManifest` with sane defaults for
//      permissions, resources, sandbox profile, and the
//      SHA-256 binary hash. The package builder signs
//      the manifest with an Ed25519 key, fills the
//      publisher fingerprint, and returns a signed
//      `AppPackage` ready for the store.
//
// Together they cover the full developer loop: build -> sign
// -> install -> launch -> uninstall.

pub mod client;
pub mod install;
pub mod manifest_builder;
pub mod package_builder;
pub mod permissions;

pub use aether_core::ipc::{ActorTrust, IpcError, IpcRequest, IpcResponse};
pub use client::AetherClient;
pub use install::{install_request, launch_request, uninstall_request, STORE_SERVICE_ID};
pub use manifest_builder::{AppManifestBuilder, ManifestBuildError};
pub use package_builder::{AppPackageBuilder, PackageBuildError};
pub use permissions::{count_permissions, permissions_to_capabilities, PermissionRequest};

/// SDK version reported to the control plane.
pub const SDK_VERSION: &str = env!("CARGO_PKG_VERSION");
