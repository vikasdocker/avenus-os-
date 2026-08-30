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
        let status = Command::new("/bin/mount").args(["-t", fstype, source, target]).status();
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

/// Brings up the virtio NIC via DHCP so the guest is reachable from the
/// host (QEMU user networking) and can reach host-side AI providers.
#[cfg(target_os = "linux")]
fn net_up() {
    let modprobe = Command::new("modprobe").arg("virtio_net").status();
    match modprobe {
        Ok(s) if s.success() => log("early-mounts", "virtio_net loaded"),
        _ => {
            // Built-in kernels have no module entry; the driver may already
            // be active, which is fine.
            log_warn("early-mounts", "virtio_net modprobe skipped (builtin?)");
        }
    }
    let _ = Command::new("modprobe").arg("psmouse").status();
    // udhcpc requires the interface administratively UP.
    let _ = Command::new("/bin/ifconfig").args(["eth0", "up"]).status();
    // Built-in kernels have no module entry; driver may already be active.
    let _ = Command::new("modprobe").arg("virtio_net").status();

    // QEMU user networking is deterministic: guest is always 10.0.2.15.
    let _ = Command::new("/bin/ifconfig").args(["eth0", "up"]).status();
    let cfg = Command::new("/bin/ifconfig")
        .args(["eth0", "10.0.2.15", "netmask", "255.255.255.0"])
        .status();
    match cfg {
        Ok(s) if s.success() => {
            log("early-mounts", "eth0 = 10.0.2.15");
            let _ = Command::new("/bin/route")
                .args(["add", "default", "gw", "10.0.2.2"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        _ => log_warn("early-mounts", "no eth0; loopback-only operation"),
    }
}

#[cfg(not(target_os = "linux"))]
fn net_up() {}

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
    let exe = if cfg!(windows) { "aether-system-core.exe" } else { "aether-system-core" };
    // PID1 resolves the core binary from PATH; the initramfs installs both.
    Command::new(exe)
        .arg(&cfg.manifest_dir)
        .env("AETHER_CONTROL_PORT", cfg.control_port.to_string())
        .env("AETHER_BIND", "0.0.0.0")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
}

/// Best-effort launch of the agent daemon (AI provider bridge). The service
/// manager does not spawn processes yet, so PID1 starts it directly.
#[cfg(target_os = "linux")]
fn spawn_agentd() -> Option<Child> {
    match Command::new("/sbin/aether-agentd").env("AETHER_BIND", "0.0.0.0").spawn() {
        Ok(child) => {
            log("services", "agent daemon started");
            Some(child)
        }
        Err(e) => {
            log_warn("services", &format!("agent daemon not started: {e}"));
            None
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn spawn_agentd() -> Option<Child> {
    None
}

/// Best-effort launch of the graphical AI shell. When `exclusive_input` is
/// set (kernel cmdline `aether=single`) the shell owns the serial console
/// as its keyboard; otherwise the console stays with the interactive shell.
#[cfg(target_os = "linux")]
fn spawn_graphical_shell(exclusive_input: bool) -> Option<Child> {
    let mut cmd = Command::new("/bin/aether-graphical-shell");
    if exclusive_input {
        cmd.env("AETHER_GFX_INPUT", "1");
    }
    match cmd.spawn() {
        Ok(child) => {
            log("ready", "graphical shell started");
            Some(child)
        }
        Err(e) => {
            log_warn("ready", &format!("graphical shell not started: {e}"));
            None
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn spawn_graphical_shell(_exclusive_input: bool) -> Option<Child> {
    None
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
    std::env::set_var("PATH", "/sbin:/usr/sbin:/bin:/usr/bin");

    let mut stage = aether_init::BootStage::EarlyMounts;

    show_banner();
    log(stage.label(), "Aether OS boot beginning");
    early_mounts();
    loopback_up();
    net_up();
    gpu_drivers();

    stage = stage.next().unwrap_or(stage);
    let cmdline = read_cmdline();
    let cfg = aether_init::BootConfig::from_cmdline(&cmdline);
    if cfg.quiet {
        QUIET.store(true, Ordering::Relaxed);
    }
    log(stage.label(), &format!("manifests={} port={}", cfg.manifest_dir, cfg.control_port));

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

    // Agent daemon first (UI talks to it), then graphical AI shell,
    // then the interactive console.
    let mut agentd = spawn_agentd();
    let mut gfx = spawn_graphical_shell(cfg.single_user);
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
        if let Some(child) = gfx.as_mut() {
            if let Ok(Some(status)) = child.try_wait() {
                log_warn("ready", &format!("graphical shell exited: {status}"));
                gfx = None;
            }
        }
        if let Some(child) = agentd.as_mut() {
            if let Ok(Some(status)) = child.try_wait() {
                log_warn("ready", &format!("agent daemon exited: {status}"));
                agentd = None;
            }
        }
        if !cfg.single_user {
            ensure_console_session(&mut console_session);
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    stage = stage.next().unwrap_or(stage);
    log_fail(stage.label(), "system-core exited; halting");
}
