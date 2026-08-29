// Aether System Manager - safe read-only OS information.
//
// Exposes system.info, system.resources, system.uptime without leaking
// secrets, tokens, or private credentials. All data is gathered via safe
// std and /proc reads, with fallbacks for non-Linux hosts.

use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// OS version and basic system identity.
pub fn system_info(services_snapshot: Option<Value>) -> Value {
    let os_version = read_os_version();
    let kernel = read_kernel_version();
    let arch = std::env::consts::ARCH.to_string();
    let os = std::env::consts::OS.to_string();
    let hostname = read_hostname();

    let mut info = json!({
        "os": os,
        "arch": arch,
        "os_version": os_version,
        "kernel_version": kernel,
        "hostname": hostname,
    });
    if let Some(services) = services_snapshot {
        if let Some(obj) = info.as_object_mut() {
            obj.insert("services".to_string(), services);
        }
    }
    info
}

/// Memory, CPU, storage usage.
pub fn system_resources(workspace_root: Option<&Path>) -> Value {
    let (mem_total_kib, mem_available_kib) = read_meminfo();
    let cpu_count = cpu_count();
    let load = read_loadavg();
    let storage = workspace_root
        .map(read_storage_for_path)
        .unwrap_or_else(|| json!({ "total_bytes": 0, "available_bytes": 0 }));

    json!({
        "cpu_count": cpu_count,
        "load_average": load,
        "memory": {
            "total_kib": mem_total_kib,
            "available_kib": mem_available_kib,
        },
        "storage": storage,
    })
}

/// Uptime information.
pub fn system_uptime(started_at: Option<SystemTime>) -> Value {
    let uptime_ms = started_at
        .and_then(|t| t.elapsed().ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or_else(|| read_uptime_ms().unwrap_or(0));

    let boot_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
        .saturating_sub(uptime_ms);

    json!({
        "uptime_ms": uptime_ms,
        "uptime_human": format_human_duration(uptime_ms),
        "boot_time_ms": boot_time,
    })
}

fn read_os_version() -> String {
    // Try /etc/os-release
    if let Ok(content) = fs::read_to_string("/etc/os-release") {
        for line in content.lines() {
            if let Some(rest) = line.strip_prefix("PRETTY_NAME=") {
                return rest.trim_matches('"').trim_matches('\'').to_string();
            }
        }
        for line in content.lines() {
            if let Some(rest) = line.strip_prefix("NAME=") {
                let name = rest.trim_matches('"');
                let version = content
                    .lines()
                    .find(|l| l.starts_with("VERSION="))
                    .and_then(|l| l.strip_prefix("VERSION="))
                    .unwrap_or("")
                    .trim_matches('"');
                if version.is_empty() {
                    return name.to_string();
                }
                return format!("{name} {version}");
            }
        }
    }
    // Windows fallback
    if cfg!(windows) {
        return format!("Windows {}", std::env::consts::OS);
    }
    std::env::consts::OS.to_string()
}

fn read_kernel_version() -> String {
    if let Ok(content) = fs::read_to_string("/proc/version") {
        return content.trim().to_string();
    }
    // Fallback to uname via sysinfo? Use env
    "unknown".to_string()
}

fn read_hostname() -> String {
    if let Ok(content) = fs::read_to_string("/proc/sys/kernel/hostname") {
        return content.trim().to_string();
    }
    if let Ok(content) = fs::read_to_string("/etc/hostname") {
        return content.trim().to_string();
    }
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "aether".to_string())
}

fn read_meminfo() -> (u64, u64) {
    if let Ok(content) = fs::read_to_string("/proc/meminfo") {
        let mut total = 0;
        let mut available = 0;
        for line in content.lines() {
            if line.starts_with("MemTotal:") {
                total = parse_kib(line);
            } else if line.starts_with("MemAvailable:") {
                available = parse_kib(line);
            }
        }
        if total > 0 {
            return (total, available);
        }
    }
    // Fallback: try to estimate via sysinfo without extra crate – return 0
    (0, 0)
}

fn parse_kib(line: &str) -> u64 {
    // Format: MemTotal:       16384256 kB
    line.split_whitespace()
        .nth(1)
        .and_then(|n| n.parse::<u64>().ok())
        .unwrap_or(0)
}

fn cpu_count() -> usize {
    // Try nproc via /proc/cpuinfo
    if let Ok(content) = fs::read_to_string("/proc/cpuinfo") {
        let count = content.lines().filter(|l| l.starts_with("processor")).count();
        if count > 0 {
            return count;
        }
    }
    // Fallback to available_parallelism
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

fn read_loadavg() -> Value {
    if let Ok(content) = fs::read_to_string("/proc/loadavg") {
        let parts: Vec<&str> = content.split_whitespace().collect();
        if parts.len() >= 3 {
            return json!({
                "1m": parts[0].parse::<f64>().unwrap_or(0.0),
                "5m": parts[1].parse::<f64>().unwrap_or(0.0),
                "15m": parts[2].parse::<f64>().unwrap_or(0.0),
            });
        }
    }
    json!({ "1m": 0.0, "5m": 0.0, "15m": 0.0 })
}

fn read_storage_for_path(path: &Path) -> Value {
    // Use statvfs via nix or just df? For minimal deps, try to read via `statvfs` using std not available.
    // Fallback: use `df` command if available, else return 0.
    // For host testing, we can estimate via `fs2` crate would be ideal but we avoid extra deps.
    // We'll try to use `std::fs::metadata` to at least confirm path exists and then return dummy storage.
    // For now, attempt to call `statvfs` via libc if unix.
    #[cfg(unix)]
    {
        // Use `statvfs` via std::process::Command df
        if let Ok(output) = std::process::Command::new("df")
            .arg("-B1")
            .arg(path)
            .output()
        {
            if output.status.success() {
                let out = String::from_utf8_lossy(&output.stdout);
                // df output: Filesystem 1B-blocks Used Available Use% Mounted on
                for line in out.lines().skip(1) {
                    let cols: Vec<&str> = line.split_whitespace().collect();
                    if cols.len() >= 4 {
                        let total = cols[1].parse::<u64>().unwrap_or(0);
                        let available = cols[3].parse::<u64>().unwrap_or(0);
                        return json!({
                            "path": path.to_string_lossy().to_string(),
                            "total_bytes": total,
                            "available_bytes": available,
                        });
                    }
                }
            }
        }
    }
    // Fallback: return 0 but still indicate path
    let total = 0;
    let available = 0;
    // Try to get at least directory existence
    let exists = path.exists();
    json!({
        "path": path.to_string_lossy().to_string(),
        "exists": exists,
        "total_bytes": total,
        "available_bytes": available,
    })
}

fn read_uptime_ms() -> Option<u64> {
    if let Ok(content) = fs::read_to_string("/proc/uptime") {
        if let Some(first) = content.split_whitespace().next() {
            if let Ok(secs) = first.parse::<f64>() {
                return Some((secs * 1000.0) as u64);
            }
        }
    }
    None
}

fn format_human_duration(ms: u64) -> String {
    let secs = ms / 1000;
    let days = secs / 86400;
    let hours = (secs % 86400) / 3600;
    let mins = (secs % 3600) / 60;
    let secs = secs % 60;
    if days > 0 {
        format!("{days}d {hours}h {mins}m")
    } else if hours > 0 {
        format!("{hours}h {mins}m {secs}s")
    } else if mins > 0 {
        format!("{mins}m {secs}s")
    } else {
        format!("{secs}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_info_has_required_fields() {
        let info = system_info(None);
        assert!(info["os"].is_string());
        assert!(info["arch"].is_string());
        assert!(info["os_version"].is_string());
    }

    #[test]
    fn system_resources_has_memory_and_cpu() {
        let res = system_resources(None);
        assert!(res["cpu_count"].is_number());
        assert!(res["memory"]["total_kib"].is_number());
    }

    #[test]
    fn system_uptime_is_non_negative() {
        let up = system_uptime(None);
        assert!(up["uptime_ms"].as_u64().unwrap_or(0) < 10_000_000_000); // less than ~115 days in ms sanity
    }

    #[test]
    fn storage_for_temp_dir() {
        let p = std::env::temp_dir();
        let s = read_storage_for_path(&p);
        assert!(s["path"].is_string());
    }
}
