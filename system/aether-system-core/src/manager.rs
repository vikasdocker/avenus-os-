// Aether System Core - service lifecycle manager.
//
// Owns the runtime state of every service. Process spawning is abstracted
// behind [`ServiceExecutor`] so lifecycle policy is testable without real
// processes and can later be backed by cgroups/seccomp sandboxes.

use crate::graph::{DependencyGraph, GraphError};
use aether_core::error::{AetherError, ErrorKind};
use aether_core::manifest::{RestartPolicy, ServiceType};
use aether_core::types::{HealthStatus, ServiceHealth, ServiceStatus};
use std::collections::BTreeMap;
use std::time::Instant;

/// Runtime handle for one service instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceHandle {
    pub service_id: String,
    pub pid: u32,
}

/// Abstraction over "how a service is actually run".
pub trait ServiceExecutor {
    /// Launches a service; returns a handle. Called only for `Process` services.
    fn start(&mut self, service_id: &str) -> Result<ServiceHandle, AetherError>;

    /// Stops a previously returned handle.
    fn stop(&mut self, handle: &ServiceHandle) -> Result<(), AetherError>;

    /// Health probe for a running handle.
    fn health(&mut self, handle: &ServiceHandle) -> Result<HealthStatus, AetherError>;
}

/// Internal services run in-process: always healthy by construction.
#[derive(Debug, Default)]
pub struct InternalExecutor;

impl ServiceExecutor for InternalExecutor {
    fn start(&mut self, _service_id: &str) -> Result<ServiceHandle, AetherError> {
        Ok(ServiceHandle { service_id: String::new(), pid: 0 })
    }

    fn stop(&mut self, _handle: &ServiceHandle) -> Result<(), AetherError> {
        Ok(())
    }

    fn health(&mut self, _handle: &ServiceHandle) -> Result<HealthStatus, AetherError> {
        Ok(HealthStatus::Healthy)
    }
}

/// Full runtime record for one managed service.
#[derive(Debug, Clone)]
struct ManagedService {
    status: ServiceStatus,
    health: HealthStatus,
    handle: Option<ServiceHandle>,
    restarts: u32,
    failures: u32,
    started_at: Option<Instant>,
}

impl ManagedService {
    fn fresh() -> Self {
        Self {
            status: ServiceStatus::Stopped,
            health: HealthStatus::Unhealthy,
            handle: None,
            restarts: 0,
            failures: 0,
            started_at: None,
        }
    }

    fn to_health_report(&self, service_id: &str) -> ServiceHealth {
        ServiceHealth {
            service_id: service_id.to_string(),
            status: self.status,
            health: self.health,
            pid: self.handle.as_ref().map(|h| h.pid),
            restarts: self.restarts,
            failures: self.failures,
            uptime_ms: self.started_at.map(|t| t.elapsed().as_millis() as u64).unwrap_or(0),
        }
    }
}

/// The system-wide service manager.
pub struct ServiceManager {
    graph: DependencyGraph,
    state: BTreeMap<String, ManagedService>,
}

impl ServiceManager {
    /// Builds a manager over a validated dependency graph.
    pub fn new(graph: DependencyGraph) -> Self {
        let state =
            graph.manifests().map(|m| (m.service_id.clone(), ManagedService::fresh())).collect();
        Self { graph, state }
    }

    /// Returns the kernel-sandbox plan the launcher must enforce
    /// before exec()ing the given service. Returns `None` for
    /// unknown services.
    ///
    /// Phase 11.4: this is the bridge between the manifest's
    /// declarative `SandboxProfile` and the future
    /// `aether-sandbox` binary. The plan is deterministic and
    /// log-friendly; the launcher writes the plan to the audit log
    /// so a post-mortem can confirm every primitive was applied.
    pub fn sandbox_plan(&self, service_id: &str) -> Option<aether_core::SandboxPlan> {
        let manifest = self.graph.manifest(service_id)?;
        Some(aether_core::plan_sandbox(manifest.sandbox_profile))
    }

    /// Iterates the sandbox plan for every known service. Used by
    /// the audit layer to confirm a launcher's intent matches the
    /// manifest declarations, and by the kernel-sandbox binary's
    /// dry-run mode.
    pub fn all_sandbox_plans(&self) -> Vec<(String, aether_core::SandboxPlan)> {
        self.graph
            .manifests()
            .map(|m| (m.service_id.clone(), aether_core::plan_sandbox(m.sandbox_profile)))
            .collect()
    }

    /// The resolved dependency graph.
    pub fn graph(&self) -> &DependencyGraph {
        &self.graph
    }

    /// Starts every service in dependency order. Fails fast: if any service
    /// cannot start, previously started services are stopped again.
    pub fn start_all(&mut self, executor: &mut dyn ServiceExecutor) -> Result<(), AetherError> {
        for service_id in self.graph.start_order().to_vec() {
            if self.start_one(executor, &service_id).is_err() {
                let _ = self.stop_all(executor);
                return Err(AetherError::service_failed(
                    &service_id,
                    "startup failed; rolled back all services",
                ));
            }
        }
        Ok(())
    }

    /// Starts a single service after verifying its dependencies are running.
    pub fn start_one(
        &mut self,
        executor: &mut dyn ServiceExecutor,
        service_id: &str,
    ) -> Result<(), AetherError> {
        let manifest = self
            .graph
            .manifest(service_id)
            .ok_or_else(|| AetherError::not_found(service_id))?
            .clone();

        for dependency in &manifest.dependencies {
            let ready =
                self.state.get(dependency).is_some_and(|s| s.status == ServiceStatus::Running);
            if !ready {
                return Err(AetherError::service_failed(
                    service_id,
                    &format!("dependency '{dependency}' is not running"),
                ));
            }
        }

        let handle = match manifest.service_type {
            ServiceType::Internal => executor.start(service_id)?,
            ServiceType::Process => {
                if manifest.command.is_none() {
                    return Err(AetherError::invalid_input(format!(
                        "service '{service_id}' has no command"
                    )));
                }
                executor.start(service_id)?
            }
        };

        if let Some(record) = self.state.get_mut(service_id) {
            record.status = ServiceStatus::Running;
            record.handle = Some(handle);
            record.started_at = Some(Instant::now());
            record.health = HealthStatus::Healthy;
        }
        Ok(())
    }

    /// Stops every service in reverse dependency order.
    pub fn stop_all(&mut self, executor: &mut dyn ServiceExecutor) -> Result<(), AetherError> {
        let mut first_error: Option<AetherError> = None;
        for service_id in self.graph.stop_order() {
            if let Err(err) = self.stop_one(executor, &service_id) {
                if first_error.is_none() {
                    first_error = Some(err);
                }
            }
        }
        match first_error {
            Some(err) => Err(err),
            None => Ok(()),
        }
    }

    /// Stops a single service.
    pub fn stop_one(
        &mut self,
        executor: &mut dyn ServiceExecutor,
        service_id: &str,
    ) -> Result<(), AetherError> {
        let record =
            self.state.get_mut(service_id).ok_or_else(|| AetherError::not_found(service_id))?;
        if let Some(handle) = record.handle.take() {
            executor.stop(&handle)?;
        }
        record.status = ServiceStatus::Stopped;
        record.health = HealthStatus::Unhealthy;
        record.started_at = None;
        Ok(())
    }

    /// Restarts a single service (stop then start).
    pub fn restart_one(
        &mut self,
        executor: &mut dyn ServiceExecutor,
        service_id: &str,
    ) -> Result<(), AetherError> {
        self.stop_one(executor, service_id)?;
        // Restart bookkeeping before bringing it back up.
        if let Some(record) = self.state.get_mut(service_id) {
            record.restarts = record.restarts.saturating_add(1);
        }
        self.start_one(executor, service_id)
    }

    /// Applies the restart policy of `service_id` after an observed failure.
    /// Returns true when a restart was scheduled/attempted.
    pub fn handle_failure(
        &mut self,
        executor: &mut dyn ServiceExecutor,
        service_id: &str,
    ) -> Result<bool, AetherError> {
        let manifest = self
            .graph
            .manifest(service_id)
            .ok_or_else(|| AetherError::not_found(service_id))?
            .clone();

        if let Some(record) = self.state.get_mut(service_id) {
            record.failures = record.failures.saturating_add(1);
            record.status = ServiceStatus::Failed;
            record.health = HealthStatus::Unhealthy;
            record.handle = None;
        } else {
            return Err(AetherError::not_found(service_id));
        }

        match manifest.restart_policy {
            RestartPolicy::Never => Ok(false),
            RestartPolicy::OnFailure | RestartPolicy::Always => {
                let limit_reached = self
                    .state
                    .get(service_id)
                    .is_some_and(|r| r.restarts >= manifest.restart_limit);
                if limit_reached {
                    return Ok(false);
                }
                if let Some(record) = self.state.get_mut(service_id) {
                    record.status = ServiceStatus::Recovering;
                }
                self.stop_one(executor, service_id)?;
                self.restart_one(executor, service_id)?;
                Ok(true)
            }
        }
    }

    /// Polls the health of every running service through the executor.
    pub fn poll_health(&mut self, executor: &mut dyn ServiceExecutor) -> Vec<ServiceHealth> {
        let ids: Vec<String> = self.graph.start_order().to_vec();
        let mut reports = Vec::with_capacity(ids.len());
        for id in ids {
            let degraded;
            if let Some(record) = self.state.get_mut(&id) {
                if let Some(handle) = record.handle.clone() {
                    match executor.health(&handle) {
                        Ok(h) => {
                            degraded = h == HealthStatus::Unhealthy;
                            record.health = h;
                        }
                        Err(_) => {
                            record.health = HealthStatus::Unhealthy;
                            degraded = true;
                        }
                    }
                    if degraded && record.status == ServiceStatus::Running {
                        record.status = ServiceStatus::Failed;
                    }
                }
                reports.push(record.to_health_report(&id));
            }
        }
        reports
    }

    /// Snapshot of the whole system.
    pub fn system_status(&self) -> aether_core::types::SystemStatus {
        let services: Vec<ServiceHealth> = self
            .graph
            .start_order()
            .iter()
            .filter_map(|id| self.state.get(id).map(|r| r.to_health_report(id)))
            .collect();
        let healthy = services.iter().all(|s| s.health == HealthStatus::Healthy);
        aether_core::types::SystemStatus {
            uptime_ms: 0,
            services,
            overall_health: if healthy { HealthStatus::Healthy } else { HealthStatus::Degraded },
        }
    }
}

/// Builds a manager from manifests in one step, mapping graph errors into
/// the shared error type.
pub fn build_manager(
    manifests: &[aether_core::manifest::ServiceManifest],
) -> Result<ServiceManager, AetherError> {
    let graph = DependencyGraph::new(manifests)
        .map_err(|err: GraphError| AetherError::new(ErrorKind::InvalidInput, err.to_string()))?;
    Ok(ServiceManager::new(graph))
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_core::manifest::{IpcAccessMode, PermissionProfile, SandboxProfile, ServiceType};
    use std::cell::RefCell;

    fn manifest(id: &str, deps: &[&str]) -> aether_core::manifest::ServiceManifest {
        aether_core::manifest::ServiceManifest {
            schema_version: "1".to_string(),
            service_id: id.to_string(),
            name: id.to_string(),
            version: "0.1.0".to_string(),
            description: String::new(),
            service_type: ServiceType::Internal,
            command: None,
            dependencies: deps.iter().map(|s| s.to_string()).collect(),
            startup_priority: 10,
            restart_policy: RestartPolicy::OnFailure,
            restart_limit: 5,
            restart_backoff_ms: 0,
            health_check: None,
            config_path: None,
            security_identity: format!("{id}.aether"),
            ipc_endpoints: Vec::new(),
            capabilities: Vec::new(),
            resource_cpu_weight: 1.0,
            resource_memory_max_kib: 1024,
            resource_process_limit: None,
            resource_io_weight: 1.0,
            requires_root: false,
            sandbox_profile: SandboxProfile::Internal,
            permission_profile: PermissionProfile::SystemInternal,
            ipc_access: IpcAccessMode::LocalPrivate,
            shutdown_timeout_ms: 100,
        }
    }

    /// Executor whose health probe fails for services listed here.
    struct FlakyExecutor {
        unhealthy: RefCell<Vec<String>>,
        next_pid: u32,
    }

    impl FlakyExecutor {
        fn new() -> Self {
            Self { unhealthy: RefCell::new(Vec::new()), next_pid: 100 }
        }
    }

    impl ServiceExecutor for FlakyExecutor {
        fn start(&mut self, _service_id: &str) -> Result<ServiceHandle, AetherError> {
            self.next_pid += 1;
            Ok(ServiceHandle { service_id: _service_id.to_string(), pid: self.next_pid })
        }

        fn stop(&mut self, _handle: &ServiceHandle) -> Result<(), AetherError> {
            Ok(())
        }

        fn health(&mut self, handle: &ServiceHandle) -> Result<HealthStatus, AetherError> {
            if self.unhealthy.borrow().contains(&handle.service_id) {
                Ok(HealthStatus::Unhealthy)
            } else {
                Ok(HealthStatus::Healthy)
            }
        }
    }

    #[test]
    fn start_stop_full_cycle_in_dependency_order() {
        let manifests = vec![manifest("top", &["base"]), manifest("base", &[])];
        let mut manager = build_manager(&manifests).unwrap_or_else(|e| panic!("{e}"));
        let mut executor = FlakyExecutor::new();
        manager.start_all(&mut executor).unwrap_or_else(|e| panic!("{e}"));
        let status = manager.system_status();
        assert_eq!(status.overall_health, HealthStatus::Healthy);
        assert_eq!(status.services.len(), 2);
        assert!(status.services.iter().all(|s| s.status == ServiceStatus::Running));

        manager.stop_all(&mut executor).unwrap_or_else(|e| panic!("{e}"));
        assert!(manager
            .system_status()
            .services
            .iter()
            .all(|s| s.status == ServiceStatus::Stopped));
    }

    #[test]
    fn start_fails_when_dependency_not_running() {
        let manifests = vec![manifest("top", &["base"]), manifest("base", &[])];
        let mut manager = build_manager(&manifests).unwrap_or_else(|e| panic!("{e}"));
        let mut executor = FlakyExecutor::new();
        match manager.start_one(&mut executor, "top") {
            Err(err) => assert_eq!(err.code, ErrorKind::ServiceFailed),
            Ok(()) => panic!("expected dependency-not-running failure"),
        }
    }

    #[test]
    fn failure_policy_triggers_restart_until_limit() {
        let mut m = manifest("svc", &[]);
        m.restart_limit = 2;
        let manifests = vec![m];
        let mut manager = build_manager(&manifests).unwrap_or_else(|e| panic!("{e}"));
        let mut executor = FlakyExecutor::new();
        manager.start_all(&mut executor).unwrap_or_else(|e| panic!("{e}"));

        assert!(manager.handle_failure(&mut executor, "svc").unwrap_or_else(|e| panic!("{e}")));
        assert!(manager.handle_failure(&mut executor, "svc").unwrap_or_else(|e| panic!("{e}")));
        // Limit reached: no more restarts.
        assert!(!manager.handle_failure(&mut executor, "svc").unwrap_or_else(|e| panic!("{e}")));
        let report = &manager.system_status().services[0];
        assert_eq!(report.failures, 3);
        assert_eq!(report.status, ServiceStatus::Failed);
    }

    #[test]
    fn health_poll_detects_unhealthy_service() {
        let manifests = vec![manifest("svc", &[])];
        let mut manager = build_manager(&manifests).unwrap_or_else(|e| panic!("{e}"));
        let mut executor = FlakyExecutor::new();
        manager.start_all(&mut executor).unwrap_or_else(|e| panic!("{e}"));
        executor.unhealthy.borrow_mut().push("svc".to_string());
        let reports = manager.poll_health(&mut executor);
        assert_eq!(reports[0].health, HealthStatus::Unhealthy);
        assert_eq!(reports[0].status, ServiceStatus::Failed);
    }

    #[test]
    fn rollback_on_failed_startup() {
        let manifests = vec![manifest("good", &[]), manifest("ghost-dep", &["missing"])];
        assert!(matches!(
            build_manager(&manifests),
            Err(AetherError { code: ErrorKind::InvalidInput, .. })
        ));
    }

    // -------- Phase 11.4 sandbox plan tests --------

    fn manifest_with_sandbox(id: &str, profile: SandboxProfile) -> aether_core::manifest::ServiceManifest {
        let mut m = manifest(id, &[]);
        m.sandbox_profile = profile;
        m
    }

    #[test]
    fn sandbox_plan_returns_none_for_unknown_service() {
        let manifests = vec![manifest("aether-agentd", &[])];
        let manager = build_manager(&manifests).unwrap_or_else(|e| panic!("{e}"));
        assert!(manager.sandbox_plan("not-a-real-service").is_none());
    }

    #[test]
    fn sandbox_plan_for_internal_has_no_primitives() {
        let manifests = vec![manifest_with_sandbox("svc", SandboxProfile::Internal)];
        let manager = build_manager(&manifests).unwrap_or_else(|e| panic!("{e}"));
        let plan = manager.sandbox_plan("svc").unwrap_or_else(|| panic!("plan missing"));
        assert_eq!(plan.profile, SandboxProfile::Internal);
        assert!(plan.namespaces.is_empty());
        assert!(plan.capabilities.is_empty());
    }

    #[test]
    fn sandbox_plan_for_system_service_drops_dangerous_caps() {
        let manifests =
            vec![manifest_with_sandbox("svc", SandboxProfile::SystemService)];
        let manager = build_manager(&manifests).unwrap_or_else(|e| panic!("{e}"));
        let plan = manager.sandbox_plan("svc").unwrap_or_else(|| panic!("plan missing"));
        assert_eq!(plan.profile, SandboxProfile::SystemService);
        assert!(plan.no_new_privs);
        // sys_admin / sys_module / sys_rawio are NOT in the allow-list.
        for cap in plan.capabilities {
            let name = cap.name();
            assert!(
                name != "sys_admin" && name != "sys_module" && name != "sys_rawio",
                "dangerous cap '{name}' leaked into SystemService plan"
            );
        }
    }

    #[test]
    fn sandbox_plan_for_restricted_app_drops_every_cap() {
        let manifests =
            vec![manifest_with_sandbox("svc", SandboxProfile::RestrictedService)];
        let manager = build_manager(&manifests).unwrap_or_else(|e| panic!("{e}"));
        let plan = manager.sandbox_plan("svc").unwrap_or_else(|| panic!("plan missing"));
        assert!(plan.capabilities.is_empty(), "restricted app must drop every cap");
        assert!(plan.namespaces.iter().any(|n| n.name() == "pid"));
        assert!(plan.namespaces.iter().any(|n| n.name() == "network"));
        assert!(plan.resources.memory_max_bytes.is_some());
    }

    #[test]
    fn all_sandbox_plans_iterates_every_service() {
        let manifests = vec![
            manifest_with_sandbox("a", SandboxProfile::Internal),
            manifest_with_sandbox("b", SandboxProfile::SystemService),
            manifest_with_sandbox("c", SandboxProfile::RestrictedService),
        ];
        let manager = build_manager(&manifests).unwrap_or_else(|e| panic!("{e}"));
        let plans = manager.all_sandbox_plans();
        assert_eq!(plans.len(), 3);
        let profiles: Vec<SandboxProfile> = plans.iter().map(|(_, p)| p.profile).collect();
        assert!(profiles.contains(&SandboxProfile::Internal));
        assert!(profiles.contains(&SandboxProfile::SystemService));
        assert!(profiles.contains(&SandboxProfile::RestrictedService));
    }
}
