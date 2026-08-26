// Aether OS PID1 binary.
//
// On Linux this runs as the first userspace process: it performs early
// mounts, reads the kernel command line, launches aether-system-core, and
// reaps children until shutdown. On non-Linux hosts (developer machines)
// it runs the same stage sequence in simulation for testing.

use std::io::Write as _;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};

/// Set from the kernel command line: suppress informational boot output.
static QUIET: AtomicBool = AtomicBool::new(false);

const RESET: &str = "\x1b[0m";
const CYAN: &str = "\x1b[36;1m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33;1m";
const RED: &str = "\x1b[31;1m";

/// Banner shown at the very start of userspace boot.
const BANNER: &str = r#"

          ___   __  __ ______ _   __ _____
         /   | / / / // ____// | / // ___/
        / /| |/ / / // __/  /  |/ / \__ \
       / ___ / /_/ // /___ / /|  / ___/ /
      /_/  |_\____//_____//_/ |_/ //____/

           AI-native operating system
"#;

fn console_out(line: &str) {
    let mut err = std::io::stderr().lock();
    let _ = writeln!(err, "{line}");
    let _ = err.flush();
}

fn log(stage: &str, message: &str) {
    if QUIET.load(Ordering::Relaxed) {
        return;
    }
    console_out(&format!("{GREEN}[ OK ]{RESET} {CYAN}{stage:<13}{RESET} {message}"));
}

fn log_warn(stage: &str, message: &str) {
    console_out(&format!("{YELLOW}[ WARN]{RESET} {CYAN}{stage:<13}{RESET} {message}"));
}

fn log_fail(stage: &str, message: &str) {
    console_out(&format!("{RED}[FAIL]{RESET} {CYAN}{stage:<13}{RESET} {message}"));
}

fn show_banner() {
    if QUIET.load(Ordering::Relaxed) {
        return;
    }
    // Clear the VGA console so only Aether output is visible.
    console_out("\x1b[2J\x1b[1;1H");
    for line in BANNER.lines() {
        console_out(&format!("{CYAN}{line}{RESET}"));
    }
    console_out("");
}

#[cfg(target_os = "linux")]
fn early_mounts() {
    // Each entry: (fstype, source, target) — sources are conventional
    // pseudo-device names required by busybox/util-linux mount.
    let mounts: &[(&str, &str, &str)] = &[
        ("proc", "proc", "/proc"),
        ("sysfs", "sysfs", "/sys"),
        ("devtmpfs", "devtmpfs", "/dev"),
        ("tmpfs", "tmpfs", "/run"),
    ];
    for (fstype, source, target) in mounts {
        let status = Command::new("/bin/mount")
            .args(["-t", fstype, source, target])
            .status();
        match status {
            Ok(s) if s.success() => {}
            Ok(s) => log_warn("early-mounts", &format!("mount {fstype} exited {s}")),
            Err(e) => log_warn("early-mounts", &format!("mount {fstype} failed: {e}")),
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn early_mounts() {
    log("early-mounts", "non-linux host: skipping filesystem mounts");
}

#[cfg(target_os = "linux")]
fn read_cmdline() -> String {
    std::fs::read_to_string("/proc/cmdline").unwrap_or_default()
}

#[cfg(not(target_os = "linux"))]
fn read_cmdline() -> String {
    std::env::var("AETHER_CMDLINE").unwrap_or_default()
}

/// Brings the loopback interface up so the local control plane can bind
/// and connect over 127.0.0.1. Tries `ip` first, then legacy `ifconfig`.
#[cfg(target_os = "linux")]
fn loopback_up() {
    let attempts: [(&str, &[&str]); 3] = [
        ("/bin/ip", &["link", "set", "lo", "up"]),
        ("/bin/ifconfig", &["lo", "127.0.0.1", "netmask", "255.0.0.0", "up"]),
        ("/sbin/ifconfig", &["lo", "up"]),
    ];
    for (prog, args) in attempts {
        if let Ok(status) = Command::new(prog).args(args).status() {
            if status.success() {
                log("early-mounts", "loopback up");
                return;
            }
        }
    }
    log_warn("early-mounts", "could not bring loopback up");
}

#[cfg(not(target_os = "linux"))]
fn loopback_up() {}

/// Best-effort KMS driver load so /dev/dri/* and a real framebuffer appear.
/// Failures are silent: the guest simply keeps the legacy console.
#[cfg(target_os = "linux")]
fn gpu_drivers() {
    for module in ["virtio_gpu", "bochs_drm"] {
        match Command::new("modprobe").arg(module).status() {
            Ok(s) if s.success() => log("early-mounts", &format!("{module} loaded")),
            _ => {}
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn gpu_drivers() {}

fn spawn_system_core(cfg: &aether_init::BootConfig) -> Result<Child, std::io::Error> {
    let exe = if cfg!(windows) {
        "aether-system-core.exe"
    } else {
        "aether-system-core"
    };
    // PID1 resolves the core binary from PATH; the initramfs installs both.
    Command::new(exe)
        .arg(&cfg.manifest_dir)
        .env("AETHER_CONTROL_PORT", cfg.control_port.to_string())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
}

/// Reaps any finished children; returns true when the system-core child exited.
fn reap(core_child: Option<&mut Child>) -> bool {
    if let Some(child) = core_child {
        if let Ok(Some(status)) = child.try_wait() {
            log("services", &format!("system-core exited: {status}"));
            return true;
        }
    }
    false
}

/// Respawns a root console shell whenever the previous one exits,
/// mirroring classic getty behaviour on /dev/console.
fn ensure_console_session(session: &mut Option<Child>) {
    let exited = match session.as_mut() {
        Some(child) => matches!(child.try_wait(), Ok(Some(_))),
        None => false,
    };
    if exited || session.is_none() {
        match Command::new("/bin/sh")
            .arg("-i")
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
        {
            Ok(child) => *session = Some(child),
            Err(e) => log("ready", &format!("cannot start console session: {e}")),
        }
    }
}

fn main() {
    // PID1 starts with an empty environment; establish the standard paths
    // so spawned services resolve against the Aether userspace layout.
    std::env::set_var(
        "PATH",
        "/sbin:/usr/sbin:/bin:/usr/bin",
    );

    let mut stage = aether_init::BootStage::EarlyMounts;

    show_banner();
    log(stage.label(), "Aether OS boot beginning");
    early_mounts();
    loopback_up();
    gpu_drivers();

    stage = stage.next().unwrap_or(stage);
    let cmdline = read_cmdline();
    let cfg = aether_init::BootConfig::from_cmdline(&cmdline);
    if cfg.quiet {
        QUIET.store(true, Ordering::Relaxed);
    }
    log(
        stage.label(),
        &format!(
            "manifests={} port={}",
            cfg.manifest_dir, cfg.control_port
        ),
    );

    stage = stage.next().unwrap_or(stage);
    log(stage.label(), "starting aether-system-core");
    let mut core = match spawn_system_core(&cfg) {
        Ok(child) => Some(child),
        Err(e) => {
            log_fail(stage.label(), &format!("failed to start system-core: {e}"));
            None
        }
    };

    stage = stage.next().unwrap_or(stage);

    // Interactive console session unless the cmdline asked for single-user
    // maintenance mode (services only, no shell).
    let mut console_session: Option<Child> = None;
    if !cfg.single_user {
        ensure_console_session(&mut console_session);
        log(stage.label(), "console ready — type 'aetherctl status'");
    }
    if core.is_some() {
        log(stage.label(), "Aether OS is live");
    } else {
        log_warn(stage.label(), "running without system-core");
    }

    // PID1 never exits by design: reap zombies and watch children.
    loop {
        if reap(core.as_mut()) {
            break;
        }
        if !cfg.single_user {
            ensure_console_session(&mut console_session);
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    stage = stage.next().unwrap_or(stage);
    log_fail(stage.label(), "system-core exited; halting");
}
