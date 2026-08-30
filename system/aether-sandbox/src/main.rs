// Aether sandbox enforcement binary.
//
// Phase 11.4 enforcement: takes a declarative `SandboxPlan` (as JSON
// on disk) and applies it before exec()ing a child process.
//
// Linux-only. The Windows / non-Linux build is a no-op stub that
// prints a clear error and exits non-zero. The declarative contract
// (`aether_core::sandbox::SandboxPlan`) is platform-independent; only
// the enforcement is Linux-only.
//
// The binary is intentionally small and auditable: parse the plan,
// apply the primitives in the order the contract specifies, then
// exec. Every step that succeeds is logged to stderr so the
// supervisor can correlate the audit record with the real kernel
// state.
//
// Apply order (deterministic):
//   1. Validate the plan (no privileged capabilities, known
//      namespaces, non-empty cgroup slice).
//   2. `prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0)` when the plan
//      requires it (always, for `SystemService` and
//      `RestrictedService`).
//   3. `unshare(CLONE_NEWNS | CLONE_NEWUSER | ...)` for the
//      namespaces the plan lists. We do this BEFORE writing the
//      cgroup slice so the cgroup write happens inside the new
//      user namespace if the plan asks for it.
//   4. Write the cgroup v2 slice (`mkdir -p` + `$$ -> <slice>`).
//   5. `capset()` to drop every ambient capability that is not in
//      the plan's whitelist.
//   6. `execvp` the child.
//
// What this binary does NOT do:
//   * Real seccomp BPF installation. The plan carries a
//     `SeccompFilterTag`; the production enforcement layer will
//     load the actual BPF program from a sidecar. For this shell
//     we log the tag and continue; the contract is that the
//     filter is "tagged for the supervisor to install before the
//     child reaches user code".
//   * Set up the cgroup controllers' actual limits (cpu.weight /
//     memory.max / etc.). The plan carries the values; the
//     production enforcement layer writes them after the slice
//     exists. For this shell we write the slice membership and
//     log the requested limits.

use std::fs;
use std::io::Write;
use std::process::ExitCode;

use aether_core::manifest::SandboxProfile;
use aether_core::sandbox::{plan_sandbox, SandboxPlan};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as plat;

#[cfg(not(target_os = "linux"))]
mod non_linux;
#[cfg(not(target_os = "linux"))]
use non_linux as plat;

const USAGE: &str = "\
aether-sandbox <--plan <file> | --profile <name>> [--] <cmd> [args...]

Apply a declarative Aether sandbox plan, then exec <cmd>.

Modes:
  --plan <file>      read a SandboxPlan JSON document from <file>
  --profile <name>   synthesize a plan from the named profile
                     (internal | system-service | restricted-service)
  --dry-run          validate and print the plan; do not apply
  --help             print this message and exit

Examples:
  aether-sandbox --plan /etc/aether/sandbox/agentd.json -- /usr/bin/aether-agentd
  aether-sandbox --profile restricted-service -- /usr/bin/aether-calculator
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let mut plan_path: Option<String> = None;
    let mut profile: Option<String> = None;
    let mut dry_run = false;
    let mut cmd_start: Option<usize> = None;
    let mut i = 1;
    while i < args.len() {
        let a = &args[i];
        match a.as_str() {
            "--plan" => {
                plan_path = args.get(i + 1).cloned();
                i += 2;
            }
            "--profile" => {
                profile = args.get(i + 1).cloned();
                i += 2;
            }
            "--dry-run" => {
                dry_run = true;
                i += 1;
            }
            "--help" | "-h" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            "--" => {
                cmd_start = Some(i + 1);
                break;
            }
            other if other.starts_with("--") => {
                eprintln!("aether-sandbox: unknown flag: {other}");
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            }
            _ => {
                cmd_start = Some(i);
                break;
            }
        }
    }
    let cmd_start = match cmd_start {
        Some(c) if c < args.len() => c,
        _ => {
            eprintln!("aether-sandbox: no command to exec");
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };

    // 1. Resolve the plan.
    let plan = match resolve_plan(plan_path.as_deref(), profile.as_deref()) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("aether-sandbox: {e}");
            return ExitCode::from(2);
        }
    };

    // 2. Validate (fail closed).
    if let Err(e) = validate(&plan) {
        eprintln!("aether-sandbox: invalid plan: {e}");
        return ExitCode::from(2);
    }

    // 3. Print + optionally apply.
    print_plan(&plan);
    if dry_run {
        return ExitCode::SUCCESS;
    }

    let cmd = &args[cmd_start..];
    plat::apply_and_exec(&plan, cmd)
}

fn resolve_plan(plan_path: Option<&str>, profile: Option<&str>) -> Result<SandboxPlan, String> {
    match (plan_path, profile) {
        (Some(_), Some(_)) => Err("--plan and --profile are mutually exclusive".to_string()),
        (None, None) => Err("one of --plan or --profile is required".to_string()),
        (Some(path), None) => {
            let text = fs::read_to_string(path)
                .map_err(|e| format!("cannot read plan file {path}: {e}"))?;
            serde_json::from_str(&text)
                .map_err(|e| format!("plan file {path} is not a valid SandboxPlan: {e}"))
        }
        (None, Some(name)) => {
            let profile = match name {
                "internal" => SandboxProfile::Internal,
                "system-service" => SandboxProfile::SystemService,
                "restricted-service" => SandboxProfile::RestrictedService,
                other => {
                    return Err(format!(
                        "unknown profile '{other}'; expected internal | system-service | restricted-service"
                    ));
                }
            };
            Ok(plan_sandbox(profile))
        }
    }
}

/// Plan validation: the launcher refuses to apply a plan that
/// asks for capabilities the security policy never allows, or
/// that points to a cgroup slice outside `aether.slice/`.
fn validate(plan: &SandboxPlan) -> Result<(), String> {
    for cap in &plan.capabilities {
        // The forbidden set: these capabilities are NEVER allowed
        // in a user-supplied plan; the launcher fails closed.
        if matches!(
            cap,
            aether_core::sandbox::LinuxCapability::SysAdmin
                | aether_core::sandbox::LinuxCapability::SysModule
                | aether_core::sandbox::LinuxCapability::SysRawio
                | aether_core::sandbox::LinuxCapability::SysBoot
                | aether_core::sandbox::LinuxCapability::LinuxImmutable
                | aether_core::sandbox::LinuxCapability::MacAdmin
                | aether_core::sandbox::LinuxCapability::MacOverride
                | aether_core::sandbox::LinuxCapability::Bpf
                | aether_core::sandbox::LinuxCapability::CheckpointRestore
        ) {
            return Err(format!("plan requests forbidden capability {}", cap.name()));
        }
    }
    if !plan.resources.cgroup_slice.starts_with("aether.slice/") {
        return Err(format!(
            "cgroup slice '{}' is not under aether.slice/",
            plan.resources.cgroup_slice
        ));
    }
    if plan.resources.cpu_weight < 1 || plan.resources.cpu_weight > 10_000 {
        return Err(format!(
            "cpu_weight {} is outside the cgroup v2 range 1..=10000",
            plan.resources.cpu_weight
        ));
    }
    if plan.resources.io_weight < 1 || plan.resources.io_weight > 10_000 {
        return Err(format!(
            "io_weight {} is outside the cgroup v2 range 1..=10000",
            plan.resources.io_weight
        ));
    }
    Ok(())
}

fn print_plan(plan: &SandboxPlan) {
    let _ = writeln!(std::io::stderr(), "aether-sandbox: profile = {:?}", plan.profile);
    let _ = writeln!(
        std::io::stderr(),
        "aether-sandbox: namespaces = [{}]",
        plan.namespaces.iter().map(|n| n.name()).collect::<Vec<_>>().join(", ")
    );
    let _ = writeln!(
        std::io::stderr(),
        "aether-sandbox: capabilities = [{}]",
        plan.capabilities.iter().map(|c| c.name()).collect::<Vec<_>>().join(", ")
    );
    let _ = writeln!(std::io::stderr(), "aether-sandbox: no_new_privs = {}", plan.no_new_privs);
    if let Some(tag) = &plan.seccomp {
        let _ = writeln!(std::io::stderr(), "aether-sandbox: seccomp = {tag}");
    } else {
        let _ = writeln!(std::io::stderr(), "aether-sandbox: seccomp = <none>");
    }
    let _ = writeln!(
        std::io::stderr(),
        "aether-sandbox: cgroup_slice = {}",
        plan.resources.cgroup_slice
    );
    let _ = writeln!(
        std::io::stderr(),
        "aether-sandbox: cpu_weight = {}, memory_max = {:?}, pids_max = {:?}, io_weight = {}",
        plan.resources.cpu_weight,
        plan.resources.memory_max_bytes,
        plan.resources.pids_max,
        plan.resources.io_weight
    );
    let _ = writeln!(std::io::stderr(), "aether-sandbox: rationale = {}", plan.rationale);
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use aether_core::sandbox::LinuxCapability;

    #[test]
    fn validate_rejects_sys_admin() {
        let mut plan = plan_sandbox(SandboxProfile::SystemService);
        plan.capabilities.push(LinuxCapability::SysAdmin);
        let err = validate(&plan).expect_err("sys_admin must be rejected");
        assert!(err.contains("forbidden capability"), "{err}");
    }

    #[test]
    fn validate_rejects_sys_module() {
        let mut plan = plan_sandbox(SandboxProfile::SystemService);
        plan.capabilities.push(LinuxCapability::SysModule);
        assert!(validate(&plan).is_err());
    }

    #[test]
    fn validate_rejects_non_aether_cgroup_slice() {
        let mut plan = plan_sandbox(SandboxProfile::SystemService);
        plan.resources.cgroup_slice = "user.slice/foo".to_string();
        let err = validate(&plan).expect_err("non-aether slice must be rejected");
        assert!(err.contains("aether.slice"), "{err}");
    }

    #[test]
    fn validate_rejects_zero_cpu_weight() {
        let mut plan = plan_sandbox(SandboxProfile::SystemService);
        plan.resources.cpu_weight = 0;
        assert!(validate(&plan).is_err());
    }

    #[test]
    fn validate_rejects_oversized_io_weight() {
        let mut plan = plan_sandbox(SandboxProfile::SystemService);
        plan.resources.io_weight = 10_001;
        assert!(validate(&plan).is_err());
    }

    #[test]
    fn validate_accepts_default_system_service_plan() {
        let plan = plan_sandbox(SandboxProfile::SystemService);
        validate(&plan).expect("default SystemService plan should validate");
    }

    #[test]
    fn validate_accepts_default_restricted_service_plan() {
        let plan = plan_sandbox(SandboxProfile::RestrictedService);
        validate(&plan).expect("default RestrictedService plan should validate");
    }

    #[test]
    fn resolve_plan_from_profile() {
        let p = resolve_plan(None, Some("restricted-service")).expect("resolve");
        assert_eq!(p.profile, SandboxProfile::RestrictedService);
    }

    #[test]
    fn resolve_plan_rejects_unknown_profile() {
        let err = resolve_plan(None, Some("nope")).expect_err("unknown profile");
        assert!(err.contains("unknown profile"), "{err}");
    }

    #[test]
    fn resolve_plan_rejects_both_flags() {
        let err = resolve_plan(Some("/tmp/p"), Some("internal")).expect_err("mutual exclusion");
        assert!(err.contains("mutually exclusive"), "{err}");
    }

    #[test]
    fn resolve_plan_rejects_neither_flag() {
        let err = resolve_plan(None, None).expect_err("must require one of --plan / --profile");
        assert!(err.contains("--plan or --profile"), "{err}");
    }

    #[test]
    fn resolve_plan_from_file() {
        let plan = plan_sandbox(SandboxProfile::SystemService);
        let mut tmp = std::env::temp_dir();
        tmp.push(format!("aether-sandbox-test-{}.json", std::process::id()));
        let text = serde_json::to_string(&plan).expect("encode");
        std::fs::write(&tmp, text).expect("write");
        let p = resolve_plan(Some(tmp.to_str().expect("utf-8")), None).expect("resolve");
        assert_eq!(p, plan);
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn print_plan_does_not_panic() {
        // Smoke test: print_plan only writes to stderr; we just
        // need to confirm it does not panic on any of the three
        // profiles.
        for profile in [
            SandboxProfile::Internal,
            SandboxProfile::SystemService,
            SandboxProfile::RestrictedService,
        ] {
            print_plan(&plan_sandbox(profile));
        }
    }
}
