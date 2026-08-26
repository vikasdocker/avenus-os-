// Aether OS PID1 - boot sequence state machine.
//
// The init binary drives the machine through deterministic boot stages.
// Stage logic lives here so it can be unit-tested on any host OS; the
// binary wires the stages to real Linux operations at runtime.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Stages of the Aether boot sequence, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BootStage {
    /// Mount pseudo filesystems and populate /dev.
    EarlyMounts,
    /// Parse kernel command line and apply boot parameters.
    KernelParams,
    /// Start the system core and its managed services.
    Services,
    /// System fully up; hand over to interactive sessions.
    Ready,
    /// Ordered shutdown in progress.
    Shutdown,
}

impl BootStage {
    pub const ALL: [BootStage; 5] = [
        BootStage::EarlyMounts,
        BootStage::KernelParams,
        BootStage::Services,
        BootStage::Ready,
        BootStage::Shutdown,
    ];

    pub fn next(&self) -> Option<Self> {
        match self {
            Self::EarlyMounts => Some(Self::KernelParams),
            Self::KernelParams => Some(Self::Services),
            Self::Services => Some(Self::Ready),
            Self::Ready => Some(Self::Shutdown),
            Self::Shutdown => None,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::EarlyMounts => "early-mounts",
            Self::KernelParams => "kernel-params",
            Self::Services => "services",
            Self::Ready => "ready",
            Self::Shutdown => "shutdown",
        }
    }
}

impl fmt::Display for BootStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// Boot configuration derived from the kernel command line / environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootConfig {
    pub manifest_dir: String,
    pub control_port: u16,
    pub quiet: bool,
    pub single_user: bool,
}

impl Default for BootConfig {
    fn default() -> Self {
        Self {
            manifest_dir: "/etc/aether/services.d".to_string(),
            control_port: 4747,
            quiet: false,
            single_user: false,
        }
    }
}

impl BootConfig {
    /// Parses an `aether=` kernel parameter value such as
    /// `quiet,port=4800,manifests=/cfg`.
    pub fn parse_param(value: &str) -> Self {
        let mut config = BootConfig::default();
        for token in value.split(',') {
            let token = token.trim();
            match token {
                "quiet" => config.quiet = true,
                "single" => config.single_user = true,
                _ => {
                    if let Some((key, val)) = token.split_once('=') {
                        match key.trim() {
                            "port" => {
                                if let Ok(p) = val.trim().parse() {
                                    config.control_port = p;
                                }
                            }
                            "manifests" => config.manifest_dir = val.trim().to_string(),
                            _ => {}
                        }
                    }
                }
            }
        }
        config
    }

    /// Extracts the `aether=` parameter from a full kernel command line.
    pub fn from_cmdline(cmdline: &str) -> Self {
        cmdline
            .split_whitespace()
            .find_map(|arg| arg.strip_prefix("aether="))
            .map(Self::parse_param)
            .unwrap_or_default()
    }
}

/// Ordered shutdown plan: services first, then core daemons.
pub fn shutdown_plan(service_ids: &[String]) -> Vec<String> {
    let mut order: Vec<String> = service_ids.to_vec();
    order.sort();
    order.reverse();
    order.push("aether-system-core".to_string());
    order
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stages_progress_in_order() {
        let mut stage = BootStage::EarlyMounts;
        assert_eq!(stage.next(), Some(BootStage::KernelParams));
        stage = stage.next().unwrap_or(stage);
        assert_eq!(stage.label(), "kernel-params");
        assert_eq!(BootStage::Shutdown.next(), None);
        assert_eq!(BootStage::ALL.len(), 5);
    }

    #[test]
    fn parses_kernel_parameter_tokens() {
        let cfg = BootConfig::parse_param("quiet,port=4800,manifests=/cfg/aether");
        assert!(cfg.quiet);
        assert_eq!(cfg.control_port, 4800);
        assert_eq!(cfg.manifest_dir, "/cfg/aether");
    }

    #[test]
    fn parses_full_cmdline_with_defaults() {
        let cfg = BootConfig::from_cmdline("root=/dev/sda1 rw console=ttyS0 aether=quiet,single");
        assert!(cfg.quiet);
        assert!(cfg.single_user);
        assert_eq!(cfg.control_port, 4747);
        assert_eq!(cfg.manifest_dir, "/etc/aether/services.d");

        let defaults = BootConfig::from_cmdline("root=/dev/sda1 ro");
        assert_eq!(defaults, BootConfig::default());
    }

    #[test]
    fn shutdown_plan_reverses_and_appends_core() {
        let plan = shutdown_plan(&["b".to_string(), "a".to_string()]);
        assert_eq!(plan, vec!["b", "a", "aether-system-core"]);
    }
}
