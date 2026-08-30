// Aether Core types - status and health enums

/// Service lifecycle status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ServiceStatus {
    Starting,
    Running,
    Stopped,
    Failed,
    Recovering,
}

impl std::fmt::Display for ServiceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Starting => write!(f, "STARTING"),
            Self::Running => write!(f, "RUNNING"),
            Self::Stopped => write!(f, "STOPPED"),
            Self::Failed => write!(f, "FAILED"),
            Self::Recovering => write!(f, "RECOVERING"),
        }
    }
}

/// Health status of a service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Healthy => write!(f, "HEALTHY"),
            Self::Degraded => write!(f, "DEGRADED"),
            Self::Unhealthy => write!(f, "UNHEALTHY"),
        }
    }
}

/// Service health report.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ServiceHealth {
    pub service_id: String,
    pub status: ServiceStatus,
    pub health: HealthStatus,
    pub pid: Option<u32>,
    pub restarts: u32,
    pub failures: u32,
    pub uptime_ms: u64,
}

/// System-wide status snapshot.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SystemStatus {
    pub uptime_ms: u64,
    pub services: Vec<ServiceHealth>,
    pub overall_health: HealthStatus,
}

impl SystemStatus {
    pub fn overall_health(&self) -> HealthStatus {
        let healthy = self.services.iter().all(|s| s.health == HealthStatus::Healthy);
        if healthy {
            HealthStatus::Healthy
        } else {
            HealthStatus::Degraded
        }
    }
}
