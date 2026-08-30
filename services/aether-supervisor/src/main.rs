// Aether Supervisor - daemon binary.
//
// Supervises a single child process described on the command line,
// restarting it according to the chosen policy with exponential backoff.

use aether_core::manifest::RestartPolicy;
use aether_supervisor::{decide, Backoff, SupervisionAction};
use std::process::Child;
use std::time::Duration;

fn parse_policy(s: &str) -> RestartPolicy {
    match s {
        "never" | "Never" => RestartPolicy::Never,
        "always" | "Always" => RestartPolicy::Always,
        _ => RestartPolicy::OnFailure,
    }
}

struct SupervisedUnit {
    command: String,
    args: Vec<String>,
    policy: RestartPolicy,
    restart_limit: u32,
    backoff: Backoff,
    restarts: u32,
    child: Option<Child>,
}

impl SupervisedUnit {
    fn launch(&mut self) -> bool {
        match std::process::Command::new(&self.command).args(&self.args).spawn() {
            Ok(child) => {
                self.child = Some(child);
                true
            }
            Err(e) => {
                eprintln!("[supervisor] spawn '{}' failed: {e}", self.command);
                false
            }
        }
    }
}

fn main() {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let mut policy = RestartPolicy::OnFailure;
    let mut limit: u32 = 5;
    let mut command_line: Vec<String> = Vec::new();

    let mut i = 0usize;
    while i < raw.len() {
        match raw[i].as_str() {
            "--policy" if i + 1 < raw.len() => {
                policy = parse_policy(&raw[i + 1]);
                i += 2;
            }
            "--limit" if i + 1 < raw.len() => {
                limit = raw[i + 1].parse::<u32>().unwrap_or(limit);
                i += 2;
            }
            "--" => {
                command_line = raw[i + 1..].to_vec();
                break;
            }
            other => {
                command_line.push(other.to_string());
                i += 1;
            }
        }
    }

    let (command, args) = match command_line.split_first() {
        Some((c, r)) => (c.clone(), r.to_vec()),
        None => {
            eprintln!("usage: aether-supervisor [--policy on-failure|always|never] [--limit N] -- <command> [args...]");
            std::process::exit(2);
        }
    };

    eprintln!("[supervisor] supervising '{command}' policy={policy:?} limit={limit}");

    let mut unit = SupervisedUnit {
        command,
        args,
        policy,
        restart_limit: limit,
        backoff: Backoff::new(200, 10_000),
        restarts: 0,
        child: None,
    };

    if !unit.launch() && unit.policy == RestartPolicy::Never {
        eprintln!("[supervisor] giving up (policy never)");
        std::process::exit(1);
    }

    loop {
        std::thread::sleep(Duration::from_millis(100));

        let exited = match unit.child.as_mut() {
            Some(child) => match child.try_wait() {
                Ok(Some(status)) => Some(status),
                Ok(None) => None,
                Err(_) => {
                    unit.child = None;
                    None
                }
            },
            None => None,
        };

        let Some(status) = exited else { continue };
        eprintln!("[supervisor] child exited: {status}");
        unit.child = None;

        let required = unit.backoff.record_failure();
        match decide(unit.policy, false, unit.restarts, unit.restart_limit, None, None) {
            SupervisionAction::Launch => {
                unit.restarts += 1;
                eprintln!(
                    "[supervisor] restarting in {:?} (attempt {}/{})",
                    required, unit.restarts, unit.restart_limit
                );
                std::thread::sleep(required);
                unit.launch();
            }
            SupervisionAction::GiveUp => {
                eprintln!("[supervisor] giving up after {} restarts", unit.restarts);
                std::process::exit(1);
            }
            SupervisionAction::Wait => {}
        }
    }
}
