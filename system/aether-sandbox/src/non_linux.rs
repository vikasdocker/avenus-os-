// Non-Linux stub for the aether-sandbox binary.
//
// On Windows / macOS the binary cannot enforce a Linux kernel
// sandbox. The plan parser, validator, and contract tests still
// run on every platform; only the kernel-level `apply_and_exec`
// step is gated to Linux.

use std::process::ExitCode;

use aether_core::sandbox::SandboxPlan;

pub fn apply_and_exec(_plan: &SandboxPlan, _cmd: &[String]) -> ExitCode {
    eprintln!("aether-sandbox: kernel-level enforcement is Linux-only");
    eprintln!("aether-sandbox: this binary built for a non-Linux target");
    eprintln!("aether-sandbox: refusing to exec to keep the contract honest");
    ExitCode::from(2)
}
