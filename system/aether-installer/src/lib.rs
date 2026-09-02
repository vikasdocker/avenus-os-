//! Installer abstraction for Aether OS.
//!
//! Provides trait-based abstractions for disk partitioning
//! (GPT+ESP), system installation, recovery image creation,
//! and rollback. Real implementations talk to the kernel's
//! block device layer; mock implementations use in-memory
//! stores for QEMU testing.

#![deny(unsafe_code)]
#![warn(missing_docs)]

use serde::{Deserialize, Serialize};
use std::fmt;

// ------------------------------------------------------------------- errors

/// Installer error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstallError {
    /// Disk not found.
    DiskNotFound(String),
    /// Insufficient space.
    InsufficientSpace {
        /// Required bytes.
        required: u64,
        /// Available bytes.
        available: u64,
    },
    /// Partition table is invalid or corrupt.
    InvalidPartitionTable(String),
    /// Filesystem operation failed.
    FilesystemError(String),
    /// Permission denied.
    PermissionDenied,
    /// Device is busy.
    DeviceBusy(String),
    /// I/O error.
    IoError(String),
}

impl fmt::Display for InstallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DiskNotFound(d) => write!(f, "disk not found: {d}"),
            Self::InsufficientSpace { required, available } => {
                write!(f, "insufficient space: need {required} bytes, have {available}")
            }
            Self::InvalidPartitionTable(s) => write!(f, "invalid partition table: {s}"),
            Self::FilesystemError(s) => write!(f, "filesystem error: {s}"),
            Self::PermissionDenied => write!(f, "permission denied"),
            Self::DeviceBusy(d) => write!(f, "device busy: {d}"),
            Self::IoError(s) => write!(f, "I/O error: {s}"),
        }
    }
}

impl std::error::Error for InstallError {}

/// Convenience result type.
pub type InstallResult<T> = Result<T, InstallError>;

// --------------------------------------------------------------- partition

/// A disk partition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Partition {
    /// Partition index (1-based).
    pub index: u32,
    /// Partition name.
    pub name: String,
    /// Partition type GUID.
    pub type_guid: String,
    /// Unique partition GUID.
    pub unique_guid: String,
    /// Start LBA.
    pub start_lba: u64,
    /// End LBA.
    pub end_lba: u64,
    /// Size in bytes.
    pub size_bytes: u64,
    /// Filesystem type, if formatted.
    pub filesystem: Option<String>,
    /// Mount point, if mounted.
    pub mount_point: Option<String>,
    /// Whether this is the ESP (EFI System Partition).
    pub is_esp: bool,
    /// Whether this is the boot partition.
    pub is_boot: bool,
    /// Whether this is the Aether root partition.
    pub is_root: bool,
    /// Whether this is the recovery partition.
    pub is_recovery: bool,
}

/// Partition type GUIDs (GPT).
pub mod partition_types {
    /// EFI System Partition.
    pub const ESP: &str = "C12A7328-F81F-11D2-BA4B-00A0C93EC93B";
    /// Linux filesystem.
    pub const LINUX_FS: &str = "0FC63DAF-8483-4772-8E79-3D69D8477DE4";
    /// Aether root partition.
    pub const AETHER_ROOT: &str = "A5B2D3E4-F6A7-8901-2345-6789ABCDEF01";
    /// Aether recovery partition.
    pub const AETHER_RECOVERY: &str = "B6C3D4E5-A7B8-9012-3456-789ABCDEF012";
}

/// A GPT disk layout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskLayout {
    /// Disk device path (e.g. "/dev/sda").
    pub device: String,
    /// Disk size in bytes.
    pub size_bytes: u64,
    /// Sector size in bytes.
    pub sector_size: u32,
    /// Partitions.
    pub partitions: Vec<Partition>,
    /// Disk GUID.
    pub disk_guid: String,
}

impl DiskLayout {
    /// Get the total used space in bytes.
    #[must_use]
    pub fn used_bytes(&self) -> u64 {
        self.partitions.iter().map(|p| p.size_bytes).sum()
    }

    /// Get the free space in bytes.
    #[must_use]
    pub fn free_bytes(&self) -> u64 {
        self.size_bytes.saturating_sub(self.used_bytes())
    }

    /// Find the ESP partition.
    #[must_use]
    pub fn esp(&self) -> Option<&Partition> {
        self.partitions.iter().find(|p| p.is_esp)
    }

    /// Find the Aether root partition.
    #[must_use]
    pub fn root(&self) -> Option<&Partition> {
        self.partitions.iter().find(|p| p.is_root)
    }

    /// Find the recovery partition.
    #[must_use]
    pub fn recovery(&self) -> Option<&Partition> {
        self.partitions.iter().find(|p| p.is_recovery)
    }
}

// --------------------------------------------------------------- installer plan

/// The installation plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallPlan {
    /// Target disk device.
    pub target_disk: String,
    /// Whether to erase the entire disk.
    pub erase_disk: bool,
    /// Partition layout to create.
    pub layout: DiskLayout,
    /// OS image version.
    pub os_version: String,
    /// OS image SHA-256 hash.
    pub os_hash: String,
    /// Whether to install the bootloader.
    pub install_bootloader: bool,
    /// Whether to create a recovery partition.
    pub create_recovery: bool,
}

// ========================================================== backend trait

/// The installer backend trait. Provides disk partitioning,
/// formatting, file copying, and bootloader installation.
pub trait InstallerBackend: Send + Sync {
    /// Get the backend name.
    fn name(&self) -> &str;

    /// Enumerate available disks.
    fn enumerate_disks(&self) -> InstallResult<Vec<DiskLayout>>;

    /// Get the layout of a specific disk.
    fn disk_layout(&self, device: &str) -> InstallResult<DiskLayout>;

    /// Create a GPT partition table on a disk.
    fn create_partition_table(&mut self, device: &str) -> InstallResult<()>;

    /// Create a partition.
    fn create_partition(
        &mut self,
        device: &str,
        name: &str,
        type_guid: &str,
        size_bytes: u64,
    ) -> InstallResult<Partition>;

    /// Delete a partition.
    fn delete_partition(&mut self, device: &str, index: u32) -> InstallResult<()>;

    /// Format a partition with a filesystem.
    fn format_partition(&mut self, device: &str, index: u32, fs_type: &str) -> InstallResult<()>;

    /// Mount a partition.
    fn mount(&self, device: &str, index: u32, mount_point: &str) -> InstallResult<()>;

    /// Unmount a partition.
    fn unmount(&self, mount_point: &str) -> InstallResult<()>;

    /// Copy a file to a mounted partition.
    fn copy_file(&self, source: &[u8], dest_path: &str) -> InstallResult<()>;

    /// Install the bootloader (GRUB/systemd-boot).
    fn install_bootloader(&mut self, device: &str, esp_mount: &str) -> InstallResult<()>;

    /// Create a recovery snapshot.
    fn create_recovery_snapshot(&self, root_mount: &str, dest: &str) -> InstallResult<()>;

    /// Apply a recovery snapshot (rollback).
    fn apply_recovery_snapshot(
        &mut self,
        snapshot_path: &str,
        root_mount: &str,
    ) -> InstallResult<()>;

    /// Validate the installation.
    fn validate(&self, device: &str) -> InstallResult<InstallValidation>;
}

/// Validation result after installation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallValidation {
    /// Whether the ESP is valid.
    pub esp_valid: bool,
    /// Whether the root partition is valid.
    pub root_valid: bool,
    /// Whether the bootloader is installed.
    pub bootloader_installed: bool,
    /// Whether the recovery partition exists.
    pub recovery_exists: bool,
    /// Any validation errors.
    pub errors: Vec<String>,
}

impl InstallValidation {
    /// Whether the installation is valid.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.esp_valid && self.root_valid && self.errors.is_empty()
    }
}

// ========================================================== mock backend

/// Mock installer backend for QEMU testing.
pub struct MockInstallerBackend {
    disks: Vec<DiskLayout>,
    next_partition_index: u32,
}

impl MockInstallerBackend {
    /// Create a new mock backend with a 256 GiB virtual disk.
    #[must_use]
    pub fn new() -> Self {
        let disk_size = 256_u64 * 1024 * 1024 * 1024; // 256 GiB
        Self {
            disks: vec![DiskLayout {
                device: "/dev/sda".into(),
                size_bytes: disk_size,
                sector_size: 512,
                partitions: Vec::new(),
                disk_guid: "aether-mock-disk-001".into(),
            }],
            next_partition_index: 1,
        }
    }
}

impl Default for MockInstallerBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl InstallerBackend for MockInstallerBackend {
    fn name(&self) -> &str {
        "mock-installer"
    }

    fn enumerate_disks(&self) -> InstallResult<Vec<DiskLayout>> {
        Ok(self.disks.clone())
    }

    fn disk_layout(&self, device: &str) -> InstallResult<DiskLayout> {
        self.disks
            .iter()
            .find(|d| d.device == device)
            .cloned()
            .ok_or_else(|| InstallError::DiskNotFound(device.into()))
    }

    fn create_partition_table(&mut self, device: &str) -> InstallResult<()> {
        let disk = self
            .disks
            .iter_mut()
            .find(|d| d.device == device)
            .ok_or_else(|| InstallError::DiskNotFound(device.into()))?;
        disk.partitions.clear();
        Ok(())
    }

    fn create_partition(
        &mut self,
        device: &str,
        name: &str,
        type_guid: &str,
        size_bytes: u64,
    ) -> InstallResult<Partition> {
        let disk = self
            .disks
            .iter_mut()
            .find(|d| d.device == device)
            .ok_or_else(|| InstallError::DiskNotFound(device.into()))?;

        if disk.free_bytes() < size_bytes {
            return Err(InstallError::InsufficientSpace {
                required: size_bytes,
                available: disk.free_bytes(),
            });
        }

        let index = self.next_partition_index;
        self.next_partition_index += 1;

        let start_lba = disk.partitions.last().map(|p| p.end_lba + 1).unwrap_or(2048);
        let sectors = size_bytes / disk.sector_size as u64;
        let end_lba = start_lba + sectors - 1;

        let is_esp = type_guid == partition_types::ESP;
        let is_root = type_guid == partition_types::AETHER_ROOT;
        let is_recovery = type_guid == partition_types::AETHER_RECOVERY;

        let partition = Partition {
            index,
            name: name.to_string(),
            type_guid: type_guid.to_string(),
            unique_guid: format!("mock-part-{index:04x}"),
            start_lba,
            end_lba,
            size_bytes,
            filesystem: None,
            mount_point: None,
            is_esp,
            is_boot: is_esp,
            is_root,
            is_recovery,
        };

        disk.partitions.push(partition.clone());
        Ok(partition)
    }

    fn delete_partition(&mut self, device: &str, index: u32) -> InstallResult<()> {
        let disk = self
            .disks
            .iter_mut()
            .find(|d| d.device == device)
            .ok_or_else(|| InstallError::DiskNotFound(device.into()))?;
        disk.partitions.retain(|p| p.index != index);
        Ok(())
    }

    fn format_partition(&mut self, device: &str, index: u32, fs_type: &str) -> InstallResult<()> {
        let disk = self
            .disks
            .iter_mut()
            .find(|d| d.device == device)
            .ok_or_else(|| InstallError::DiskNotFound(device.into()))?;
        let part = disk.partitions.iter_mut().find(|p| p.index == index).ok_or_else(|| {
            InstallError::InvalidPartitionTable(format!("partition {index} not found"))
        })?;
        part.filesystem = Some(fs_type.to_string());
        Ok(())
    }

    fn mount(&self, _device: &str, _index: u32, _mount_point: &str) -> InstallResult<()> {
        Ok(())
    }

    fn unmount(&self, _mount_point: &str) -> InstallResult<()> {
        Ok(())
    }

    fn copy_file(&self, _source: &[u8], _dest_path: &str) -> InstallResult<()> {
        Ok(())
    }

    fn install_bootloader(&mut self, _device: &str, _esp_mount: &str) -> InstallResult<()> {
        Ok(())
    }

    fn create_recovery_snapshot(&self, _root_mount: &str, _dest: &str) -> InstallResult<()> {
        Ok(())
    }

    fn apply_recovery_snapshot(
        &mut self,
        _snapshot_path: &str,
        _root_mount: &str,
    ) -> InstallResult<()> {
        Ok(())
    }

    fn validate(&self, device: &str) -> InstallResult<InstallValidation> {
        let disk = self
            .disks
            .iter()
            .find(|d| d.device == device)
            .ok_or_else(|| InstallError::DiskNotFound(device.into()))?;

        let esp_valid = disk.esp().map(|p| p.filesystem.is_some()).unwrap_or(false);
        let root_valid = disk.root().map(|p| p.filesystem.is_some()).unwrap_or(false);

        Ok(InstallValidation {
            esp_valid,
            root_valid,
            bootloader_installed: esp_valid,
            recovery_exists: disk.recovery().is_some(),
            errors: Vec::new(),
        })
    }
}

/// Plan an Aether installation on a disk.
#[must_use]
pub fn plan_installation(device: &str, disk_size: u64) -> InstallPlan {
    let esp_size = 512_u64 * 1024 * 1024; // 512 MiB
    let recovery_size = 2_u64 * 1024 * 1024 * 1024; // 2 GiB
    let boot_size = 1024_u64 * 1024 * 1024; // 1 GiB
    let root_size = disk_size - esp_size - recovery_size - boot_size;

    let mut layout = DiskLayout {
        device: device.to_string(),
        size_bytes: disk_size,
        sector_size: 512,
        partitions: Vec::new(),
        disk_guid: "aether-install-001".into(),
    };

    // Build partition list inline.
    let mut next_index = 1u32;
    let mut start = 2048u64;

    let make_part =
        |index: &mut u32, name: &str, guid: &str, size: u64, start_lba: &mut u64| -> Partition {
            let sectors = size / 512;
            let end = *start_lba + sectors - 1;
            let p = Partition {
                index: *index,
                name: name.to_string(),
                type_guid: guid.to_string(),
                unique_guid: format!("install-{index:04x}"),
                start_lba: *start_lba,
                end_lba: end,
                size_bytes: size,
                filesystem: None,
                mount_point: None,
                is_esp: guid == partition_types::ESP,
                is_boot: guid == partition_types::ESP,
                is_root: guid == partition_types::AETHER_ROOT,
                is_recovery: guid == partition_types::AETHER_RECOVERY,
            };
            *start_lba = end + 1;
            *index += 1;
            p
        };

    layout.partitions.push(make_part(
        &mut next_index,
        "ESP",
        partition_types::ESP,
        esp_size,
        &mut start,
    ));
    layout.partitions.push(make_part(
        &mut next_index,
        "boot",
        partition_types::LINUX_FS,
        boot_size,
        &mut start,
    ));
    layout.partitions.push(make_part(
        &mut next_index,
        "aether-root",
        partition_types::AETHER_ROOT,
        root_size,
        &mut start,
    ));
    layout.partitions.push(make_part(
        &mut next_index,
        "recovery",
        partition_types::AETHER_RECOVERY,
        recovery_size,
        &mut start,
    ));

    InstallPlan {
        target_disk: device.to_string(),
        erase_disk: true,
        layout,
        os_version: "0.1.0".into(),
        os_hash: String::new(),
        install_bootloader: true,
        create_recovery: true,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn plan_installation_creates_four_partitions() {
        let plan = plan_installation("/dev/sda", 256_u64 * 1024 * 1024 * 1024);
        assert_eq!(plan.layout.partitions.len(), 4);
        assert!(plan.layout.esp().is_some());
        assert!(plan.layout.root().is_some());
        assert!(plan.layout.recovery().is_some());
    }

    #[test]
    fn mock_installer_enumerate_disks() {
        let backend = MockInstallerBackend::new();
        let disks = backend.enumerate_disks().unwrap();
        assert_eq!(disks.len(), 1);
        assert_eq!(disks[0].device, "/dev/sda");
    }

    #[test]
    fn mock_installer_create_partition() {
        let mut backend = MockInstallerBackend::new();
        backend.create_partition_table("/dev/sda").unwrap();
        let part = backend
            .create_partition("/dev/sda", "test", partition_types::LINUX_FS, 1024 * 1024 * 1024)
            .unwrap();
        assert_eq!(part.index, 1);
        assert_eq!(part.name, "test");
    }

    #[test]
    fn mock_installer_insufficient_space() {
        let mut backend = MockInstallerBackend::new();
        backend.create_partition_table("/dev/sda").unwrap();
        let result = backend.create_partition(
            "/dev/sda",
            "huge",
            partition_types::LINUX_FS,
            1024_u64 * 1024 * 1024 * 1024 * 1024, // 1 TiB — too big
        );
        assert!(result.is_err());
    }

    #[test]
    fn mock_installer_format_partition() {
        let mut backend = MockInstallerBackend::new();
        backend.create_partition_table("/dev/sda").unwrap();
        backend
            .create_partition("/dev/sda", "test", partition_types::LINUX_FS, 1024 * 1024)
            .unwrap();
        backend.format_partition("/dev/sda", 1, "ext4").unwrap();
        let layout = backend.disk_layout("/dev/sda").unwrap();
        assert_eq!(layout.partitions[0].filesystem.as_deref(), Some("ext4"));
    }

    #[test]
    fn mock_installer_validate() {
        let mut backend = MockInstallerBackend::new();
        backend.create_partition_table("/dev/sda").unwrap();
        backend
            .create_partition("/dev/sda", "ESP", partition_types::ESP, 512 * 1024 * 1024)
            .unwrap();
        backend.format_partition("/dev/sda", 1, "vfat").unwrap();
        backend
            .create_partition(
                "/dev/sda",
                "root",
                partition_types::AETHER_ROOT,
                10_u64 * 1024 * 1024 * 1024,
            )
            .unwrap();
        backend.format_partition("/dev/sda", 2, "ext4").unwrap();
        let result = backend.validate("/dev/sda").unwrap();
        assert!(result.is_valid());
        assert!(result.esp_valid);
        assert!(result.root_valid);
    }

    #[test]
    fn disk_layout_free_bytes() {
        let layout = DiskLayout {
            device: "test".into(),
            size_bytes: 1000,
            sector_size: 512,
            partitions: vec![Partition {
                index: 1,
                name: "a".into(),
                type_guid: "".into(),
                unique_guid: "".into(),
                start_lba: 0,
                end_lba: 100,
                size_bytes: 100,
                filesystem: None,
                mount_point: None,
                is_esp: false,
                is_boot: false,
                is_root: false,
                is_recovery: false,
            }],
            disk_guid: "".into(),
        };
        assert_eq!(layout.used_bytes(), 100);
        assert_eq!(layout.free_bytes(), 900);
    }

    #[test]
    fn install_error_display() {
        let e = InstallError::DiskNotFound("sdb".into());
        assert!(e.to_string().contains("sdb"));
    }

    #[test]
    fn partition_type_guids_are_distinct() {
        let guids = [
            partition_types::ESP,
            partition_types::LINUX_FS,
            partition_types::AETHER_ROOT,
            partition_types::AETHER_RECOVERY,
        ];
        let mut sorted = guids.to_vec();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), 4);
    }
}
