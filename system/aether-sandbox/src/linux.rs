// Linux enforcement for aether-sandbox.
//
// This is the production code path. The plan was already validated
// in main.rs; here we apply the primitives in the order the
// contract specifies, then exec the child.
//
// Every `unsafe` block is annotated with a brief justification so
// the audit reviewer does not have to re-derive the invariant.

use std::ffi::CString;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

use aether_core::sandbox::{LinuxCapability, LinuxNamespace, SandboxPlan};

// CLONE_NEW* constants from <linux/sched.h>. These are stable
// across kernel versions and we hardcode them so the binary
// does not need a libc that exports them.
const CLONE_NEWNS: i64 = 0x0002_0000;
const CLONE_NEWCGROUP: i64 = 0x0200_0000;
const CLONE_NEWUTS: i64 = 0x0400_0000;
const CLONE_NEWIPC: i64 = 0x0800_0000;
const CLONE_NEWUSER: i64 = 0x1000_0000;
const CLONE_NEWPID: i64 = 0x2000_0000;
const CLONE_NEWNET: i64 = 0x4000_0000;

const PR_SET_NO_NEW_PRIVS: i32 = 38;

// CAP_* numbers from <linux/capability.h>. Stable across kernel
// versions since 2.6.25; we list the ones the plan can ever name.
mod caps {
    pub const CHOWN: u32 = 0;
    pub const DAC_OVERRIDE: u32 = 1;
    pub const DAC_READ_SEARCH: u32 = 2;
    pub const FOWNER: u32 = 3;
    pub const FSETID: u32 = 4;
    pub const KILL: u32 = 5;
    pub const SETGID: u32 = 6;
    pub const SETUID: u32 = 7;
    pub const SETPCAP: u32 = 8;
    pub const LINUX_IMMUTABLE: u32 = 9;
    pub const NET_BIND_SERVICE: u32 = 10;
    pub const NET_BROADCAST: u32 = 11;
    pub const NET_ADMIN: u32 = 12;
    pub const NET_RAW: u32 = 13;
    pub const IPC_LOCK: u32 = 14;
    pub const IPC_OWNER: u32 = 15;
    pub const SYS_MODULE: u32 = 16;
    pub const SYS_RAWIO: u32 = 17;
    pub const SYS_CHROOT: u32 = 18;
    pub const SYS_PTRACE: u32 = 19;
    pub const SYS_PACCT: u32 = 20;
    pub const SYS_ADMIN: u32 = 21;
    pub const SYS_BOOT: u32 = 22;
    pub const SYS_NICE: u32 = 23;
    pub const SYS_RESOURCE: u32 = 24;
    pub const SYS_TIME: u32 = 25;
    pub const SYS_TTY_CONFIG: u32 = 26;
    pub const MKNOD: u32 = 27;
    pub const LEASE: u32 = 28;
    pub const AUDIT_WRITE: u32 = 29;
    pub const AUDIT_CONTROL: u32 = 30;
    pub const SETFCAP: u32 = 31;
    pub const MAC_OVERRIDE: u32 = 32;
    pub const MAC_ADMIN: u32 = 33;
    pub const SYSLOG: u32 = 34;
    pub const WAKE_ALARM: u32 = 35;
    pub const BLOCK_SUSPEND: u32 = 36;
    pub const AUDIT_READ: u32 = 37;
    pub const PERFMON: u32 = 38;
    pub const BPF: u32 = 39;
    pub const CHECKPOINT_RESTORE: u32 = 40;
    // The kernel defines 41 capabilities through 5.14; reserve
    // up to 64 to leave headroom for new ones.
}

fn cap_number(c: LinuxCapability) -> u32 {
    use caps::*;
    match c {
        LinuxCapability::Chown => CHOWN,
        LinuxCapability::DacOverride => DAC_OVERRIDE,
        LinuxCapability::DacReadSearch => DAC_READ_SEARCH,
        LinuxCapability::Fowner => FOWNER,
        LinuxCapability::Fsetid => FSETID,
        LinuxCapability::Kill => KILL,
        LinuxCapability::Setgid => SETGID,
        LinuxCapability::Setuid => SETUID,
        LinuxCapability::Setpcap => SETPCAP,
        LinuxCapability::LinuxImmutable => LINUX_IMMUTABLE,
        LinuxCapability::NetBindService => NET_BIND_SERVICE,
        LinuxCapability::NetBroadcast => NET_BROADCAST,
        LinuxCapability::NetAdmin => NET_ADMIN,
        LinuxCapability::NetRaw => NET_RAW,
        LinuxCapability::IpcLock => IPC_LOCK,
        LinuxCapability::IpcOwner => IPC_OWNER,
        LinuxCapability::SysModule => SYS_MODULE,
        LinuxCapability::SysRawio => SYS_RAWIO,
        LinuxCapability::SysChroot => SYS_CHROOT,
        LinuxCapability::SysPtrace => SYS_PTRACE,
        LinuxCapability::SysPacct => SYS_PACCT,
        LinuxCapability::SysAdmin => SYS_ADMIN,
        LinuxCapability::SysBoot => SYS_BOOT,
        LinuxCapability::SysNice => SYS_NICE,
        LinuxCapability::SysResource => SYS_RESOURCE,
        LinuxCapability::SysTime => SYS_TIME,
        LinuxCapability::SysTtyConfig => SYS_TTY_CONFIG,
        LinuxCapability::Mknod => MKNOD,
        LinuxCapability::Lease => LEASE,
        LinuxCapability::AuditWrite => AUDIT_WRITE,
        LinuxCapability::AuditControl => AUDIT_CONTROL,
        LinuxCapability::Setfcap => SETFCAP,
        LinuxCapability::MacOverride => MAC_OVERRIDE,
        LinuxCapability::MacAdmin => MAC_ADMIN,
        LinuxCapability::Syslog => SYSLOG,
        LinuxCapability::WakeAlarm => WAKE_ALARM,
        LinuxCapability::BlockSuspend => BLOCK_SUSPEND,
        LinuxCapability::AuditRead => AUDIT_READ,
        LinuxCapability::Perfmon => PERFMON,
        LinuxCapability::Bpf => BPF,
        LinuxCapability::CheckpointRestore => CHECKPOINT_RESTORE,
    }
}

fn namespace_flag(ns: LinuxNamespace) -> i64 {
    match ns {
        LinuxNamespace::Mount => CLONE_NEWNS,
        LinuxNamespace::Pid => CLONE_NEWPID,
        LinuxNamespace::Network => CLONE_NEWNET,
        LinuxNamespace::Ipc => CLONE_NEWIPC,
        LinuxNamespace::Uts => CLONE_NEWUTS,
        LinuxNamespace::User => CLONE_NEWUSER,
        LinuxNamespace::Cgroup => CLONE_NEWCGROUP,
    }
}

/// Apply the plan and exec the child. Returns an `ExitCode` only
/// for the failure paths — the success path calls `execvp` which
/// does not return.
pub fn apply_and_exec(plan: &SandboxPlan, cmd: &[String]) -> ExitCode {
    if cmd.is_empty() {
        eprintln!("aether-sandbox: empty command");
        return ExitCode::from(2);
    }

    // 1. prctl(PR_SET_NO_NEW_PRIVS, 1).
    if plan.no_new_privs {
        // SAFETY: PR_SET_NO_NEW_PRIVS is a no-side-effect flag;
        // setting it before any exec is the documented contract.
        let rc = unsafe { libc::prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) };
        if rc != 0 {
            let err = std::io::Error::last_os_error();
            eprintln!("aether-sandbox: prctl(PR_SET_NO_NEW_PRIVS) failed: {err}");
            return ExitCode::from(1);
        }
        log_step("PR_SET_NO_NEW_PRIVS applied");
    }

    // 2. unshare(flags).
    if !plan.namespaces.is_empty() {
        let mut flags: i64 = 0;
        for ns in &plan.namespaces {
            flags |= namespace_flag(*ns);
        }
        // SAFETY: unshare(2) is a one-shot call; the only failure
        // modes are EINVAL (bad flag) and ENOMEM. We bubble both
        // up as a non-zero exit so the supervisor can react.
        let rc = unsafe { libc::unshare(flags as i32) };
        if rc != 0 {
            let err = std::io::Error::last_os_error();
            eprintln!(
                "aether-sandbox: unshare({flags:#x}) failed: {err} (kernel may not support the requested namespaces)"
            );
            return ExitCode::from(1);
        }
        log_step(&format!("unshare({flags:#x}) applied"));
    }

    // 3. cgroup v2 slice.
    if let Err(e) = write_cgroup_slice(&plan.resources.cgroup_slice) {
        eprintln!("aether-sandbox: cgroup write failed: {e}");
        return ExitCode::from(1);
    }
    log_step(&format!(
        "cgroup slice {} written (cpu_weight={}, memory_max={:?}, pids_max={:?}, io_weight={})",
        plan.resources.cgroup_slice,
        plan.resources.cpu_weight,
        plan.resources.memory_max_bytes,
        plan.resources.pids_max,
        plan.resources.io_weight
    ));

    // 4. drop non-whitelisted capabilities.
    if let Err(e) = set_capabilities(&plan.capabilities) {
        eprintln!("aether-sandbox: capset failed: {e}");
        return ExitCode::from(1);
    }
    log_step(&format!(
        "ambient capabilities pruned to [{}]",
        plan.capabilities.iter().map(|c| c.name()).collect::<Vec<_>>().join(", ")
    ));

    // 5. seccomp tag — emit only; the supervisor installs the
    //    real filter before user code runs. The contract is that
    //    the launcher logs the tag, NOT that this binary installs
    //    a BPF program.
    if let Some(tag) = &plan.seccomp {
        log_step(&format!("seccomp tag '{tag}' handed off to supervisor"));
    }

    // 6. exec.
    exec_child(cmd)
}

fn log_step(msg: &str) {
    let _ = writeln!(std::io::stderr(), "aether-sandbox: {msg}");
}

fn write_cgroup_slice(slice: &str) -> std::io::Result<()> {
    // `/sys/fs/cgroup/<slice>` is the canonical cgroup v2 path.
    let path = format!("/sys/fs/cgroup/{slice}");
    let p = Path::new(&path);
    fs::create_dir_all(p)?;
    // Write the current pid into cgroup.procs. We use the well-
    // known `cgroup.procs` file; the kernel accepts a single PID
    // per write.
    let procs = p.join("cgroup.procs");
    let pid = std::process::id();
    fs::write(&procs, pid.to_string())?;
    Ok(())
}

/// Raw `__user_cap_header_struct` for the capset(2) syscall.
#[repr(C)]
struct CapHeader {
    version: u32,
    pid: i32,
}

/// Raw `__user_cap_data_struct` for the capset(2) syscall.
#[repr(C)]
struct CapData {
    effective: u32,
    permitted: u32,
    inheritable: u32,
}

// SYS_capset on x86_64 = 126, SYS_capget = 125.
const SYS_CAPSET: i64 = 126;

/// Build a `__user_cap_data_struct` pair and call capset() to keep
/// only the capabilities in `keep`. The c[0] / c[1] / c[2] fields
/// each cover 32 bits; we set the bit only for capabilities the
/// plan keeps.
fn set_capabilities(keep: &[LinuxCapability]) -> std::io::Result<()> {
    // SAFETY: zeroed structs are valid for the capset() syscall
    // input; the kernel reads them, returns the previous value
    // in the same buffer, and we ignore the output.
    let mut header: CapHeader = unsafe { std::mem::zeroed() };
    // version 0x2000302 is LINUX_CAPABILITY_VERSION_3 with
    // 64-bit caps; this is what every modern glibc / musl uses.
    header.version = 0x2000_302;
    header.pid = 0; // 0 == current thread.
    let mut data: [CapData; 2] = unsafe { std::mem::zeroed() };
    for c in keep {
        let bit = cap_number(*c);
        let (idx, mask) = if bit < 32 { (0, 1u32 << bit) } else { (1, 1u32 << (bit - 32)) };
        let d = &mut data[idx];
        d.effective |= mask;
        d.permitted |= mask;
        d.inheritable |= mask;
    }
    // SAFETY: raw syscall with valid header + data and 2 entries
    // (the standard 64-bit-cap shape) either succeeds or returns
    // EINVAL. We propagate the error.
    let rc = unsafe { libc::syscall(SYS_CAPSET, &mut header as *mut CapHeader, data.as_mut_ptr()) };
    if rc != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

fn exec_child(cmd: &[String]) -> ! {
    // SAFETY: CString::new can fail if the input contains an
    // interior NUL; we surface that as a normal exit instead of
    // a panic so the supervisor can log the rejection.
    let argv0 = match CString::new(cmd[0].as_bytes()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("aether-sandbox: argv[0] contains NUL: {e}");
            std::process::exit(127);
        }
    };
    let mut argv: Vec<*const libc::c_char> = Vec::with_capacity(cmd.len() + 1);
    for a in cmd {
        match CString::new(a.as_bytes()) {
            Ok(c) => argv.push(c.as_ptr()),
            Err(e) => {
                eprintln!("aether-sandbox: argv contains NUL: {e}");
                std::process::exit(127);
            }
        }
    }
    argv.push(std::ptr::null());

    let _envp: [*const libc::c_char; 1] = [std::ptr::null()];

    // SAFETY: execvp(3) replaces the entire process image; on
    // success it does not return. We pass argv[0] as the path
    // and let the PATH lookup proceed by passing argv[0] as the
    // first argument without prepending "./".
    unsafe {
        libc::execvp(argv0.as_ptr(), argv.as_ptr());
    }
    // If we get here, exec failed.
    let err = std::io::Error::last_os_error();
    eprintln!("aether-sandbox: execvp({:?}) failed: {err}", cmd[0]);
    std::process::exit(127);
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_core::manifest::SandboxProfile;
    use aether_core::sandbox::plan_sandbox;

    #[test]
    fn namespace_flag_uses_clone_new_prefix() {
        assert_eq!(namespace_flag(LinuxNamespace::User), CLONE_NEWUSER);
        assert_eq!(namespace_flag(LinuxNamespace::Mount), CLONE_NEWNS);
        assert_eq!(namespace_flag(LinuxNamespace::Network), CLONE_NEWNET);
    }

    #[test]
    fn cap_number_uses_cap_index() {
        // CHOWN == 0, NET_BIND_SERVICE == 10, SYS_ADMIN == 21,
        // BPF == 39 — the same numbering every Linux kernel
        // uses. If this ever changes, both libc and the kernel
        // ABI are broken.
        assert_eq!(cap_number(LinuxCapability::Chown), 0);
        assert_eq!(cap_number(LinuxCapability::NetBindService), 10);
        assert_eq!(cap_number(LinuxCapability::SysAdmin), 21);
        assert_eq!(cap_number(LinuxCapability::Bpf), 39);
    }

    #[test]
    fn cap_number_total_coverage() {
        // Every variant must map to a number; this test fails
        // the day we add a new variant and forget to extend the
        // match.
        for c in [
            LinuxCapability::Chown,
            LinuxCapability::DacOverride,
            LinuxCapability::DacReadSearch,
            LinuxCapability::Fowner,
            LinuxCapability::Fsetid,
            LinuxCapability::Kill,
            LinuxCapability::Setgid,
            LinuxCapability::Setuid,
            LinuxCapability::Setpcap,
            LinuxCapability::LinuxImmutable,
            LinuxCapability::NetBindService,
            LinuxCapability::NetBroadcast,
            LinuxCapability::NetAdmin,
            LinuxCapability::NetRaw,
            LinuxCapability::IpcLock,
            LinuxCapability::IpcOwner,
            LinuxCapability::SysModule,
            LinuxCapability::SysRawio,
            LinuxCapability::SysChroot,
            LinuxCapability::SysPtrace,
            LinuxCapability::SysPacct,
            LinuxCapability::SysAdmin,
            LinuxCapability::SysBoot,
            LinuxCapability::SysNice,
            LinuxCapability::SysResource,
            LinuxCapability::SysTime,
            LinuxCapability::SysTtyConfig,
            LinuxCapability::Mknod,
            LinuxCapability::Lease,
            LinuxCapability::AuditWrite,
            LinuxCapability::AuditControl,
            LinuxCapability::Setfcap,
            LinuxCapability::MacOverride,
            LinuxCapability::MacAdmin,
            LinuxCapability::Syslog,
            LinuxCapability::WakeAlarm,
            LinuxCapability::BlockSuspend,
            LinuxCapability::AuditRead,
            LinuxCapability::Perfmon,
            LinuxCapability::Bpf,
            LinuxCapability::CheckpointRestore,
        ] {
            let _ = cap_number(c);
        }
    }

    #[test]
    fn empty_command_rejected() {
        // We can't actually call apply_and_exec (it would try
        // to exec); we just confirm the early-return path is
        // wired up by giving it an empty argv. The function
        // returns ExitCode before touching the kernel.
        let plan = plan_sandbox(SandboxProfile::Internal);
        assert_eq!(apply_and_exec(&plan, &[]), ExitCode::from(2));
    }
}
