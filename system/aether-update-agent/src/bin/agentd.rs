//! Aether update agent daemon.
//!
//! Phase 12.8. The daemon owns a
//! `UpdateAgent<NullApplyEngine>`,
//! installs the default retry
//! policies, and waits for a
//! `SignedUpdate` on stdin. When a
//! well-formed envelope arrives, it
//! drives the agent through the
//! `Download -> Verify -> Stage ->
//! Snapshot -> Apply` sequence and
//! prints the resulting audit log
//! to stdout.
//!
//! For now the daemon uses the
//! `NullApplyEngine`; the real disk
//! backend (`FilesystemApplyEngine`
//! in the library, plus a future
//! `RealFsApplyEngine`) is a
//! drop-in. The contract is the
//! `ApplyEngine` trait, the
//! `UpdateAgent<E>`, and the
//! `AgentAuditEvent` log; this
//! binary is the supervisor that
//! drives the state machine.
//!
//! Usage:
//!   aether-update-agentd \
//!     --plan path/to/plan.json \
//!     --now-ms 1700000000000
//!
//! Or via stdin: pipe a serialized
//! `UpdatePlan` JSON and the
//! daemon will run it.

#![deny(unsafe_code)]
#![warn(missing_docs)]

use std::io::{self, BufRead};
use std::path::PathBuf;
use std::process::ExitCode;

use aether_update_agent::{
    ApplyStep, NullApplyEngine, UpdateAgent,
};
use aether_update_core::plan::UpdatePlan;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let parsed = parse_args(&args);
    let parsed = match parsed {
        Ok(p) => p,
        Err(e) => {
            eprintln!("usage error: {e}");
            return ExitCode::from(2);
        }
    };

    // Read the plan: either from a
    // path (--plan) or from stdin.
    let plan = match read_plan(&parsed) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("plan read error: {e}");
            return ExitCode::from(3);
        }
    };

    let mut agent: UpdateAgent<NullApplyEngine> = UpdateAgent::new(NullApplyEngine);
    agent.install_default_policies();
    agent.accept(plan, parsed.now_ms);

    println!(
        "aether-update-agentd: plan accepted (target={} version={} kind={:?} action={:?})",
        agent
            .status()
            .current_plan()
            .map(|p| p.target.as_str())
            .unwrap_or("?"),
        agent
            .status()
            .current_plan()
            .map(|p| p.version.as_str())
            .unwrap_or("?"),
        agent.status().current_plan().map(|p| p.kind),
        agent.status().current_plan().map(|p| p.action),
    );

    // Drive every step. The
    // NullApplyEngine succeeds
    // for every step, so the
    // full sequence completes and
    // we end in `Done`.
    for step in [
        ApplyStep::Download,
        ApplyStep::Verify,
        ApplyStep::Stage,
        ApplyStep::Snapshot,
        ApplyStep::Apply,
    ] {
        if let Err(e) = agent.run_step(step, parsed.now_ms) {
            eprintln!(
                "aether-update-agentd: step {step:?} failed: {e}; rolling back"
            );
            agent.fail_and_rollback(step, parsed.now_ms);
            print_audit(&agent);
            return ExitCode::from(4);
        }
    }

    print_audit(&agent);
    ExitCode::SUCCESS
}

struct Cli {
    plan_path: Option<PathBuf>,
    now_ms: u64,
}

fn parse_args(args: &[String]) -> Result<Cli, String> {
    let mut plan_path: Option<PathBuf> = None;
    let mut now_ms: u64 = 0;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--plan" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or_else(|| "--plan requires a value".to_string())?;
                plan_path = Some(PathBuf::from(value));
            }
            "--now-ms" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or_else(|| "--now-ms requires a value".to_string())?;
                now_ms = value
                    .parse::<u64>()
                    .map_err(|e| format!("invalid --now-ms: {e}"))?;
            }
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
        i += 1;
    }
    Ok(Cli { plan_path, now_ms })
}

fn print_help() {
    println!("aether-update-agentd");
    println!();
    println!("USAGE:");
    println!("    aether-update-agentd --plan <PATH> --now-ms <MILLIS>");
    println!("    aether-update-agentd < plan.json");
    println!();
    println!("OPTIONS:");
    println!("    --plan <PATH>     Read the UpdatePlan JSON from <PATH>.");
    println!("    --now-ms <MILLIS> Wall-clock timestamp for the state machine.");
    println!("    -h, --help        Print this help.");
}

fn read_plan(cli: &Cli) -> Result<UpdatePlan, String> {
    let json = if let Some(path) = &cli.plan_path {
        std::fs::read_to_string(path)
            .map_err(|e| format!("read {}: {e}", path.display()))?
    } else {
        let mut buf = String::new();
        io::stdin()
            .lock()
            .read_line(&mut buf)
            .map_err(|e| format!("stdin: {e}"))?;
        buf
    };
    serde_json::from_str(&json).map_err(|e| format!("json: {e}"))
}

fn print_audit<E: aether_update_agent::ApplyEngine>(agent: &UpdateAgent<E>) {
    println!();
    println!("== audit log ==");
    for (i, event) in agent.audit().iter().enumerate() {
        println!("{i:>3}: {event:?}");
    }
    println!();
    println!("== final state ==");
    println!("stage: {:?}", agent.status().stage());
    println!(
        "history entries: {}",
        agent.history().len()
    );
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn parse_help_flag() {
        // --help calls process::exit;
        // we just sanity-check the
        // unknown-arg path.
        let result = parse_args(&[
            "aether-update-agentd".to_string(),
            "--bogus".to_string(),
        ]);
        assert!(result.is_err());
    }

    #[test]
    fn parse_plan_and_now() {
        let result = parse_args(&[
            "aether-update-agentd".to_string(),
            "--plan".to_string(),
            "plan.json".to_string(),
            "--now-ms".to_string(),
            "123".to_string(),
        ])
        .unwrap();
        assert_eq!(result.plan_path, Some(PathBuf::from("plan.json")));
        assert_eq!(result.now_ms, 123);
    }

    #[test]
    fn parse_plan_missing_value_errors() {
        let result = parse_args(&["aether-update-agentd".to_string(), "--plan".to_string()]);
        assert!(result.is_err());
    }
}
