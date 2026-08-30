// Aether System Core - dependency graph resolution.
//
// Validates that every declared dependency exists, rejects circular
// dependencies, and produces a deterministic start order.

use aether_core::error::{AetherError, ErrorKind};
use aether_core::manifest::ServiceManifest;
use std::collections::{BTreeMap, VecDeque};

/// Resolved service dependency graph with deterministic ordering.
#[derive(Debug, Clone)]
pub struct DependencyGraph {
    manifests: BTreeMap<String, ServiceManifest>,
    start_order: Vec<String>,
}

/// Errors detected while resolving the dependency graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphError {
    MissingDependency { service: String, dependency: String },
    CircularDependency { cycle: Vec<String> },
    DuplicateServiceId { service: String },
}

impl std::fmt::Display for GraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingDependency { service, dependency } => {
                write!(f, "service '{service}' depends on missing service '{dependency}'")
            }
            Self::CircularDependency { cycle } => {
                write!(f, "circular dependency detected: {}", cycle.join(" -> "))
            }
            Self::DuplicateServiceId { service } => {
                write!(f, "duplicate service id '{service}'")
            }
        }
    }
}

impl std::error::Error for GraphError {}

impl DependencyGraph {
    /// Builds and validates the graph from an unordered set of manifests.
    pub fn new(manifests: &[ServiceManifest]) -> Result<Self, GraphError> {
        let mut map: BTreeMap<String, ServiceManifest> = BTreeMap::new();
        for manifest in manifests {
            if map.insert(manifest.service_id.clone(), manifest.clone()).is_some() {
                return Err(GraphError::DuplicateServiceId {
                    service: manifest.service_id.clone(),
                });
            }
        }

        // Every dependency must exist.
        for manifest in map.values() {
            for dependency in &manifest.dependencies {
                if !map.contains_key(dependency) {
                    return Err(GraphError::MissingDependency {
                        service: manifest.service_id.clone(),
                        dependency: dependency.clone(),
                    });
                }
            }
        }

        // Kahn topological sort; startup_priority breaks ties deterministically.
        let mut indegree: BTreeMap<String, usize> = map
            .keys()
            .map(|id| {
                let count = map[id].dependencies.iter().filter(|d| map.contains_key(*d)).count();
                (id.clone(), count)
            })
            .collect();
        let dependents: BTreeMap<String, Vec<String>> = {
            let mut m: BTreeMap<String, Vec<String>> = BTreeMap::new();
            for manifest in map.values() {
                for dep in &manifest.dependencies {
                    m.entry(dep.clone()).or_default().push(manifest.service_id.clone());
                }
            }
            m
        };

        let mut ready: Vec<&ServiceManifest> =
            indegree.iter().filter(|(_, deg)| **deg == 0).map(|(id, _)| &map[id]).collect();
        ready.sort_by_key(|m| (m.startup_priority, m.service_id.clone()));

        let mut queue: VecDeque<String> = ready.iter().map(|m| m.service_id.clone()).collect();
        let mut start_order: Vec<String> = Vec::with_capacity(map.len());

        while let Some(id) = queue.pop_front() {
            start_order.push(id.clone());
            if let Some(children) = dependents.get(&id) {
                let mut unlocked: Vec<&ServiceManifest> = Vec::new();
                for child in children {
                    let entry =
                        indegree.get_mut(child).ok_or_else(|| GraphError::MissingDependency {
                            service: id.clone(),
                            dependency: child.clone(),
                        })?;
                    *entry = entry.saturating_sub(1);
                    if *entry == 0 {
                        unlocked.push(&map[child]);
                    }
                }
                unlocked.sort_by_key(|m| (m.startup_priority, m.service_id.clone()));
                for m in unlocked {
                    queue.push_back(m.service_id.clone());
                }
            }
        }

        if start_order.len() != map.len() {
            let mut remaining: Vec<String> =
                map.keys().filter(|id| !start_order.contains(id)).cloned().collect();
            remaining.sort();
            return Err(GraphError::CircularDependency { cycle: remaining });
        }

        Ok(Self { manifests: map, start_order })
    }

    /// Deterministic start order: dependencies before dependents.
    pub fn start_order(&self) -> &[String] {
        &self.start_order
    }

    /// Reverse of the start order; used for shutdown.
    pub fn stop_order(&self) -> Vec<String> {
        let mut order = self.start_order.clone();
        order.reverse();
        order
    }

    /// Look up a manifest by service id.
    pub fn manifest(&self, service_id: &str) -> Option<&ServiceManifest> {
        self.manifests.get(service_id)
    }

    /// All manifests in the graph.
    pub fn manifests(&self) -> impl Iterator<Item = &ServiceManifest> {
        self.manifests.values()
    }

    /// Number of services in the graph.
    pub fn len(&self) -> usize {
        self.manifests.len()
    }

    /// True when the graph contains no services.
    pub fn is_empty(&self) -> bool {
        self.manifests.is_empty()
    }
}

/// Convenience conversion so graph errors flow through the shared error type.
impl From<GraphError> for AetherError {
    fn from(err: GraphError) -> Self {
        AetherError::new(ErrorKind::InvalidInput, err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_core::manifest::{
        IpcAccessMode, PermissionProfile, RestartPolicy, SandboxProfile, ServiceType,
    };

    fn manifest(id: &str, deps: &[&str], priority: u32) -> ServiceManifest {
        ServiceManifest {
            schema_version: "1".to_string(),
            service_id: id.to_string(),
            name: id.to_string(),
            version: "0.1.0".to_string(),
            description: String::new(),
            service_type: ServiceType::Internal,
            command: None,
            dependencies: deps.iter().map(|s| s.to_string()).collect(),
            startup_priority: priority,
            restart_policy: RestartPolicy::OnFailure,
            restart_limit: 3,
            restart_backoff_ms: 10,
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

    #[test]
    fn resolves_dependency_order() {
        let manifests = vec![
            manifest("shell", &["compositor", "session"], 30),
            manifest("session", &["core"], 20),
            manifest("core", &[], 10),
            manifest("compositor", &["core"], 20),
        ];
        let graph = DependencyGraph::new(&manifests).unwrap_or_else(|e| panic!("{e}"));
        let order = graph.start_order();
        assert_eq!(order[0], "core");
        let pos = |sid: &str| order.iter().position(|x| x == sid).unwrap_or(usize::MAX);
        assert!(pos("core") < pos("session"));
        assert!(pos("core") < pos("compositor"));
        assert!(pos("compositor") < pos("shell"));
        assert!(pos("session") < pos("shell"));
    }

    #[test]
    fn missing_dependency_rejected() {
        let manifests = vec![manifest("orphan", &["ghost"], 1)];
        match DependencyGraph::new(&manifests) {
            Err(GraphError::MissingDependency { service, dependency }) => {
                assert_eq!(service, "orphan");
                assert_eq!(dependency, "ghost");
            }
            other => panic!("expected missing dependency error, got {other:?}"),
        }
    }

    #[test]
    fn circular_dependency_rejected() {
        let manifests =
            vec![manifest("a", &["b"], 1), manifest("b", &["c"], 1), manifest("c", &["a"], 1)];
        assert!(matches!(
            DependencyGraph::new(&manifests),
            Err(GraphError::CircularDependency { .. })
        ));
    }

    #[test]
    fn duplicate_ids_rejected() {
        let manifests = vec![manifest("dup", &[], 1), manifest("dup", &[], 2)];
        assert!(matches!(
            DependencyGraph::new(&manifests),
            Err(GraphError::DuplicateServiceId { .. })
        ));
    }

    #[test]
    fn stop_order_is_reverse_of_start_order() {
        let manifests = vec![manifest("base", &[], 1), manifest("top", &["base"], 2)];
        let graph = DependencyGraph::new(&manifests).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(graph.stop_order(), vec!["top".to_string(), "base".to_string()]);
    }
}
