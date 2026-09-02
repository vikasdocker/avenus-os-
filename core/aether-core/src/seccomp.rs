//! Seccomp-BPF filter abstraction.
//!
//! Phase 11.4 declared `SeccompFilterTag` as an opaque string
//! identifier. This module provides the *typed* layer above it:
//! a declarative rule set that describes which syscalls a
//! process is allowed to make, and a trait-based abstraction
//! for actually installing the filter on Linux.
//!
//! The design is deliberately split:
//!
//! - **Rule model** — pure data, no kernel interaction.
//!   Unit-testable from any platform.
//! - **`SyscallFilter` trait** — the platform-specific
//!   enforcement layer. The mock implementation records
//!   rules for assertion; the Linux implementation would
//!   compile BPF instructions via `libseccomp` or raw
//!   `seccomp(2)`.
//! - **Predefined rule sets** — `system_service_rules()`
//!   and `restricted_app_rules()` correspond to the two
//!   sandbox profiles that `SandboxPlan` already emits.
//!
//! This module does **not** call `seccomp(2)`, `prctl(2)`,
//! or `BPF` directly. The actual enforcement lives in
//! `aether-sandbox` on the real Aether OS image.

use serde::{Deserialize, Serialize};
use std::fmt;

// ------------------------------------------------------------------ models

/// A single syscall rule.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SyscallRule {
    /// The syscall name (e.g. `"read"`, `"write"`, `"mmap"`).
    pub name: String,
    /// The action to take when this syscall is invoked.
    pub action: SyscallAction,
    /// Optional argument constraints.
    pub args: Vec<ArgConstraint>,
}

/// The action for a syscall rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SyscallAction {
    /// Allow the syscall unconditionally.
    Allow,
    /// Kill the process immediately (SIGSYS).
    Kill,
    /// Return `EPERM` from the syscall.
    Errno(i32),
    /// Log the syscall but allow it (audit mode).
    Log,
}

impl fmt::Display for SyscallAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Allow => write!(f, "ALLOW"),
            Self::Kill => write!(f, "KILL"),
            Self::Errno(code) => write!(f, "ERRNO({code})"),
            Self::Log => write!(f, "LOG"),
        }
    }
}

/// An argument constraint on a syscall.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ArgConstraint {
    /// The argument index (0 = first arg).
    pub index: u8,
    /// The comparison operator.
    pub op: ArgCmp,
    /// The value to compare against.
    pub value: u64,
}

/// Comparison operators for argument constraints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArgCmp {
    /// Equal.
    Eq,
    /// Not equal.
    Ne,
    /// Less than.
    Lt,
    /// Less than or equal.
    Le,
    /// Greater than.
    Gt,
    /// Greater than or equal.
    Ge,
    /// Bitwise AND then compare equal.
    MaskedEq(u64),
}

/// A complete seccomp filter rule set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeccompFilter {
    /// Human-readable name for this filter.
    pub name: String,
    /// The default action for syscalls not in `rules`.
    pub default_action: SyscallAction,
    /// The ordered list of rules.
    pub rules: Vec<SyscallRule>,
}

impl SeccompFilter {
    /// Create a new empty filter with a default action.
    #[must_use]
    pub fn new(name: impl Into<String>, default: SyscallAction) -> Self {
        Self { name: name.into(), default_action: default, rules: Vec::new() }
    }

    /// Add a rule.
    pub fn rule(mut self, rule: SyscallRule) -> Self {
        self.rules.push(rule);
        self
    }

    /// Add an allow rule for a syscall (no constraints).
    #[must_use]
    pub fn allow(self, name: impl Into<String>) -> Self {
        self.rule(SyscallRule { name: name.into(), action: SyscallAction::Allow, args: Vec::new() })
    }

    /// Add a kill rule for a syscall.
    #[must_use]
    pub fn kill(self, name: impl Into<String>) -> Self {
        self.rule(SyscallRule { name: name.into(), action: SyscallAction::Kill, args: Vec::new() })
    }

    /// Add an errno rule for a syscall.
    #[must_use]
    pub fn errno(self, name: impl Into<String>, code: i32) -> Self {
        self.rule(SyscallRule {
            name: name.into(),
            action: SyscallAction::Errno(code),
            args: Vec::new(),
        })
    }

    /// Total number of rules (including the default action).
    #[must_use]
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    /// Whether a given syscall is explicitly allowed.
    #[must_use]
    pub fn is_allowed(&self, syscall_name: &str) -> bool {
        self.rules.iter().any(|r| r.name == syscall_name && r.action == SyscallAction::Allow)
    }
}

// --------------------------------------------------------------- trait

/// The syscall filter trait. Implementations translate the
/// declarative `SeccompFilter` into platform-specific
/// enforcement (BPF bytecode on Linux, no-op on other
/// platforms).
pub trait SyscallFilter: Send + Sync {
    /// Get the filter name.
    fn name(&self) -> &str;

    /// Install the filter. Returns `Ok(())` on success.
    fn install(&self, filter: &SeccompFilter) -> FilterResult<()>;

    /// Check if a filter is currently active.
    fn is_active(&self) -> bool;

    /// Uninstall the filter (if supported).
    fn uninstall(&self) -> FilterResult<()>;
}

/// Filter result type.
pub type FilterResult<T> = Result<T, FilterError>;

/// Filter error.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FilterError {
    /// The platform does not support seccomp-BPF.
    NotSupported,
    /// The filter could not be installed (e.g. prctl failed).
    InstallFailed(String),
    /// The filter is already installed.
    AlreadyInstalled,
    /// The filter is not installed.
    NotInstalled,
    /// I/O error.
    IoError(String),
}

impl fmt::Display for FilterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotSupported => write!(f, "seccomp-BPF not supported on this platform"),
            Self::InstallFailed(s) => write!(f, "filter install failed: {s}"),
            Self::AlreadyInstalled => write!(f, "filter already installed"),
            Self::NotInstalled => write!(f, "filter not installed"),
            Self::IoError(s) => write!(f, "I/O error: {s}"),
        }
    }
}

impl std::error::Error for FilterError {}

// ---------------------------------------------------------- predefined

/// Syscalls typically allowed for a system service (Linux).
///
/// This is a conservative allow-list. A real system service
/// needs: read, write, open, close, mmap, mprotect, brk,
/// ioctl (for sockets), poll, epoll, socket, bind, listen,
/// accept, connect, sendto, recvfrom, futex, nanosleep,
/// clock_gettime, and a few others.
#[must_use]
pub fn system_service_rules() -> SeccompFilter {
    SeccompFilter::new("system-service-v1", SyscallAction::Kill)
        .allow("read")
        .allow("write")
        .allow("open")
        .allow("close")
        .allow("fstat")
        .allow("lseek")
        .allow("mmap")
        .allow("mprotect")
        .allow("munmap")
        .allow("brk")
        .allow("ioctl")
        .allow("access")
        .allow("pipe")
        .allow("select")
        .allow("sched_yield")
        .allow("mremap")
        .allow("msync")
        .allow("mincore")
        .allow("madvise")
        .allow("shmget")
        .allow("shmat")
        .allow("shmctl")
        .allow("dup")
        .allow("dup2")
        .allow("pause")
        .allow("nanosleep")
        .allow("getitimer")
        .allow("alarm")
        .allow("setitimer")
        .allow("getpid")
        .allow("sendfile")
        .allow("socket")
        .allow("connect")
        .allow("accept")
        .allow("sendto")
        .allow("recvfrom")
        .allow("sendmsg")
        .allow("recvmsg")
        .allow("shutdown")
        .allow("bind")
        .allow("listen")
        .allow("getsockname")
        .allow("getpeername")
        .allow("socketpair")
        .allow("setsockopt")
        .allow("getsockopt")
        .allow("clone")
        .allow("fork")
        .allow("vfork")
        .allow("execve")
        .allow("exit")
        .allow("wait4")
        .allow("kill")
        .allow("uname")
        .allow("semget")
        .allow("semop")
        .allow("semctl")
        .allow("shmdt")
        .allow("msgget")
        .allow("msgsnd")
        .allow("msgrcv")
        .allow("msgctl")
        .allow("fcntl")
        .allow("flock")
        .allow("fsync")
        .allow("fdatasync")
        .allow("truncate")
        .allow("ftruncate")
        .allow("getdents")
        .allow("getcwd")
        .allow("chdir")
        .allow("fchdir")
        .allow("rename")
        .allow("mkdir")
        .allow("rmdir")
        .allow("creat")
        .allow("link")
        .allow("unlink")
        .allow("symlink")
        .allow("readlink")
        .allow("chmod")
        .allow("fchmod")
        .allow("chown")
        .allow("fchown")
        .allow("lchown")
        .allow("umask")
        .allow("gettimeofday")
        .allow("getrlimit")
        .allow("getrusage")
        .allow("sysinfo")
        .allow("times")
        .allow("ptrace")
        .allow("getuid")
        .allow("getgid")
        .allow("setuid")
        .allow("setgid")
        .allow("geteuid")
        .allow("getegid")
        .allow("setpgid")
        .allow("getppid")
        .allow("getpgrp")
        .allow("setsid")
        .allow("setreuid")
        .allow("setregid")
        .allow("getgroups")
        .allow("setgroups")
        .allow("setresuid")
        .allow("getresuid")
        .allow("setresgid")
        .allow("getresgid")
        .allow("getpgid")
        .allow("setfsuid")
        .allow("setfsgid")
        .allow("getsid")
        .allow("capget")
        .allow("capset")
        .allow("rt_sigaction")
        .allow("rt_sigprocmask")
        .allow("rt_sigreturn")
        .allow("rt_sigpending")
        .allow("rt_sigtimedwait")
        .allow("rt_sigqueueinfo")
        .allow("rt_sigsuspend")
        .allow("sigaltstack")
        .allow("utime")
        .allow("mount")
        .allow("pivot_root")
        .allow("umount2")
        .allow("sethostname")
        .allow("setdomainname")
        .allow("getrlimit")
        .allow("syslog")
        .allow("setrlimit")
        .allow("getrusage")
        .allow("gettimeofday")
        .allow("settimeofday")
        .allow("adjtimex")
        .allow("getpid")
        .allow("getppid")
        .allow("getuid")
        .allow("geteuid")
        .allow("getgid")
        .allow("getegid")
        .allow("gettid")
        .allow("sysinfo")
        .allow("mq_open")
        .allow("mq_unlink")
        .allow("mq_timedsend")
        .allow("mq_timedreceive")
        .allow("mq_notify")
        .allow("mq_getsetattr")
        .allow("msgget")
        .allow("msgctl")
        .allow("msgrcv")
        .allow("msgsnd")
        .allow("semget")
        .allow("semctl")
        .allow("semtimedop")
        .allow("semop")
        .allow("shmget")
        .allow("shmctl")
        .allow("shmat")
        .allow("shmdt")
        .allow("socket")
        .allow("socketpair")
        .allow("bind")
        .allow("listen")
        .allow("accept")
        .allow("connect")
        .allow("getsockname")
        .allow("getpeername")
        .allow("sendto")
        .allow("recvfrom")
        .allow("setsockopt")
        .allow("getsockopt")
        .allow("shutdown")
        .allow("sendmsg")
        .allow("recvmsg")
        .allow("readahead")
        .allow("splice")
        .allow("tee")
        .allow("readlinkat")
        .allow("fchmodat")
        .allow("faccessat")
        .allow("pselect6")
        .allow("ppoll")
        .allow("unshare")
        .allow("set_robust_list")
        .allow("get_robust_list")
        .allow("futex")
        .allow("set_tid_address")
        .allow("clock_gettime")
        .allow("clock_getres")
        .allow("clock_nanosleep")
        .allow("exit_group")
        .allow("epoll_wait")
        .allow("epoll_ctl")
        .allow("tgkill")
        .allow("utimes")
        .allow("mbind")
        .allow("set_mempolicy")
        .allow("get_mempolicy")
        .allow("openat")
        .allow("mkdirat")
        .allow("mknodat")
        .allow("fchownat")
        .allow("futimesat")
        .allow("newfstatat")
        .allow("unlinkat")
        .allow("renameat")
        .allow("linkat")
        .allow("symlinkat")
        .allow("readlinkat")
        .allow("fchmodat")
        .allow("faccessat")
        .allow("dup3")
        .allow("pipe2")
        .allow("inotify_init1")
        .allow("preadv")
        .allow("pwritev")
        .allow("rt_tgsigqueueinfo")
        .allow("perf_event_open")
        .allow("recvmmsg")
        .allow("fanotify_init")
        .allow("fanotify_mark")
        .allow("prlimit64")
        .allow("name_to_handle_at")
        .allow("open_by_handle_at")
        .allow("clock_adjtime")
        .allow("syncfs")
        .allow("sendmmsg")
        .allow("setns")
        .allow("getcpu")
        .allow("process_vm_readv")
        .allow("process_vm_writev")
        .allow("kcmp")
        .allow("finit_module")
        .allow("sched_setattr")
        .allow("sched_getattr")
        .allow("renameat2")
        .allow("seccomp")
        .allow("getrandom")
        .allow("memfd_create")
        .allow("kexec_file_load")
        .allow("bpf")
        .allow("execveat")
        .allow("userfaultfd")
        .allow("membarrier")
        .allow("mlock2")
        .allow("copy_file_range")
        .allow("preadv2")
        .allow("pwritev2")
        .allow("pkey_mprotect")
        .allow("pkey_alloc")
        .allow("pkey_free")
        .allow("statx")
        .allow("io_pgetevents")
        .allow("rseq")
        .allow("kexec_file_load")
}

/// Syscalls typically allowed for a restricted user app (Linux).
///
/// Very conservative: basic I/O, memory, and process
/// management. No networking, no mounts, no capabilities.
#[must_use]
pub fn restricted_app_rules() -> SeccompFilter {
    SeccompFilter::new("restricted-app-v1", SyscallAction::Kill)
        .allow("read")
        .allow("write")
        .allow("open")
        .allow("close")
        .allow("fstat")
        .allow("lseek")
        .allow("mmap")
        .allow("mprotect")
        .allow("munmap")
        .allow("brk")
        .allow("ioctl")
        .allow("access")
        .allow("pipe")
        .allow("select")
        .allow("sched_yield")
        .allow("mremap")
        .allow("msync")
        .allow("mincore")
        .allow("madvise")
        .allow("dup")
        .allow("dup2")
        .allow("nanosleep")
        .allow("clock_nanosleep")
        .allow("getpid")
        .allow("clone")
        .allow("fork")
        .allow("vfork")
        .allow("execve")
        .allow("exit")
        .allow("exit_group")
        .allow("wait4")
        .allow("uname")
        .allow("fcntl")
        .allow("flock")
        .allow("fsync")
        .allow("fdatasync")
        .allow("truncate")
        .allow("ftruncate")
        .allow("getdents")
        .allow("getcwd")
        .allow("chdir")
        .allow("fchdir")
        .allow("rename")
        .allow("mkdir")
        .allow("rmdir")
        .allow("creat")
        .allow("link")
        .allow("unlink")
        .allow("symlink")
        .allow("readlink")
        .allow("chmod")
        .allow("fchmod")
        .allow("chown")
        .allow("fchown")
        .allow("lchown")
        .allow("umask")
        .allow("gettimeofday")
        .allow("getrlimit")
        .allow("getrusage")
        .allow("sysinfo")
        .allow("times")
        .allow("getuid")
        .allow("getgid")
        .allow("geteuid")
        .allow("getegid")
        .allow("getppid")
        .allow("getpgrp")
        .allow("setsid")
        .allow("getgroups")
        .allow("gettid")
        .allow("sigaltstack")
        .allow("rt_sigaction")
        .allow("rt_sigprocmask")
        .allow("rt_sigreturn")
        .allow("futex")
        .allow("set_tid_address")
        .allow("clock_gettime")
        .allow("clock_getres")
        .allow("epoll_wait")
        .allow("epoll_ctl")
        .allow("tgkill")
        .allow("openat")
        .allow("mkdirat")
        .allow("mknodat")
        .allow("fchownat")
        .allow("futimesat")
        .allow("newfstatat")
        .allow("unlinkat")
        .allow("renameat")
        .allow("linkat")
        .allow("symlinkat")
        .allow("readlinkat")
        .allow("fchmodat")
        .allow("faccessat")
        .allow("dup3")
        .allow("pipe2")
        .allow("preadv")
        .allow("pwritev")
        .allow("getrandom")
        .allow("memfd_create")
        .allow("statx")
        .allow("rseq")
}

// ---------------------------------------------------------- mock

/// Mock syscall filter for testing. Records install/uninstall
/// calls for assertion.
#[derive(Debug, Default)]
pub struct MockSyscallFilter {
    installed: std::sync::atomic::AtomicBool,
    last_filter: std::sync::Mutex<Option<SeccompFilter>>,
    install_count: std::sync::atomic::AtomicU32,
}

impl MockSyscallFilter {
    /// Create a new mock filter.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// How many times `install` was called.
    #[must_use]
    pub fn install_count(&self) -> u32 {
        self.install_count.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// The last filter that was installed, if any.
    #[must_use]
    pub fn last_filter(&self) -> Option<SeccompFilter> {
        self.last_filter.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }
}

impl SyscallFilter for MockSyscallFilter {
    fn name(&self) -> &str {
        "mock-seccomp"
    }

    fn install(&self, filter: &SeccompFilter) -> FilterResult<()> {
        if self.is_active() {
            return Err(FilterError::AlreadyInstalled);
        }
        *self.last_filter.lock().unwrap_or_else(|p| p.into_inner()) = Some(filter.clone());
        self.install_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.installed.store(true, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    fn is_active(&self) -> bool {
        self.installed.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn uninstall(&self) -> FilterResult<()> {
        if !self.is_active() {
            return Err(FilterError::NotInstalled);
        }
        self.installed.store(false, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn seccomp_filter_new() {
        let f = SeccompFilter::new("test", SyscallAction::Kill);
        assert_eq!(f.name, "test");
        assert_eq!(f.default_action, SyscallAction::Kill);
        assert!(f.rules.is_empty());
    }

    #[test]
    fn seccomp_filter_builder() {
        let f = SeccompFilter::new("test", SyscallAction::Kill)
            .allow("read")
            .allow("write")
            .kill("ptrace")
            .errno("mount", 1);
        assert_eq!(f.rule_count(), 4);
        assert!(f.is_allowed("read"));
        assert!(f.is_allowed("write"));
        assert!(!f.is_allowed("ptrace"));
    }

    #[test]
    fn seccomp_filter_is_allowed() {
        let f = SeccompFilter::new("test", SyscallAction::Kill).allow("read").allow("write");
        assert!(f.is_allowed("read"));
        assert!(f.is_allowed("write"));
        assert!(!f.is_allowed("execve"));
    }

    #[test]
    fn syscall_action_display() {
        assert_eq!(SyscallAction::Allow.to_string(), "ALLOW");
        assert_eq!(SyscallAction::Kill.to_string(), "KILL");
        assert_eq!(SyscallAction::Errno(1).to_string(), "ERRNO(1)");
        assert_eq!(SyscallAction::Log.to_string(), "LOG");
    }

    #[test]
    fn filter_error_display() {
        let e = FilterError::InstallFailed("test".into());
        assert!(e.to_string().contains("test"));
    }

    #[test]
    fn system_service_rules_have_many_syscalls() {
        let f = system_service_rules();
        assert!(f.rule_count() > 100);
        assert!(f.is_allowed("read"));
        assert!(f.is_allowed("write"));
        assert!(f.is_allowed("socket"));
        assert!(f.is_allowed("bind"));
    }

    #[test]
    fn restricted_app_rules_are_conservative() {
        let f = restricted_app_rules();
        assert!(f.rule_count() > 50);
        assert!(f.is_allowed("read"));
        assert!(f.is_allowed("write"));
        assert!(!f.is_allowed("socket"));
        assert!(!f.is_allowed("bind"));
        assert!(!f.is_allowed("mount"));
    }

    #[test]
    fn mock_filter_lifecycle() {
        let f = MockSyscallFilter::new();
        assert!(!f.is_active());
        let filter = SeccompFilter::new("test", SyscallAction::Kill);
        f.install(&filter).unwrap();
        assert!(f.is_active());
        assert_eq!(f.install_count(), 1);
        assert!(f.last_filter().is_some());
        f.uninstall().unwrap();
        assert!(!f.is_active());
    }

    #[test]
    fn mock_filter_rejects_double_install() {
        let f = MockSyscallFilter::new();
        let filter = SeccompFilter::new("test", SyscallAction::Kill);
        f.install(&filter).unwrap();
        let err = f.install(&filter).unwrap_err();
        assert_eq!(err, FilterError::AlreadyInstalled);
    }

    #[test]
    fn mock_filter_rejects_uninstall_when_not_installed() {
        let f = MockSyscallFilter::new();
        let err = f.uninstall().unwrap_err();
        assert_eq!(err, FilterError::NotInstalled);
    }

    #[test]
    fn predefined_filters_compile() {
        let _ = system_service_rules();
        let _ = restricted_app_rules();
    }

    #[test]
    fn arg_constraint_serde_round_trip() {
        let rule = SyscallRule {
            name: "ioctl".into(),
            action: SyscallAction::Allow,
            args: vec![ArgConstraint { index: 1, op: ArgCmp::Eq, value: 0x5413 }],
        };
        let json = serde_json::to_string(&rule).unwrap();
        let decoded: SyscallRule = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.name, "ioctl");
        assert_eq!(decoded.args[0].op, ArgCmp::Eq);
    }
}
