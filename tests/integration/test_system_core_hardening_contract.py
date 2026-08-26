from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[2]
MAX_RESTART_LIMIT = 25
MAX_PROCESS_LIMIT = 4096
MAX_CPU_WEIGHT = 10000
MAX_IO_WEIGHT = 10000
MAX_SHUTDOWN_TIMEOUT_MS = 120000


class SystemCoreHardeningContractTests(unittest.TestCase):
    def test_manifest_security_profiles_are_admissible(self) -> None:
        for manifest in load_manifests().values():
            self.assertIn(
                manifest["sandbox_profile"],
                {"internal", "system-service", "restricted-service"},
            )
            self.assertIn(
                manifest["permission_profile"],
                {"system-internal", "service-runtime", "developer-control"},
            )
            self.assertEqual(manifest["ipc_access"], "local-private")
            if manifest["service_type"] == "process":
                self.assertNotEqual(manifest["sandbox_profile"], "internal")

    def test_manifest_resource_bounds_are_admissible(self) -> None:
        for service_id, manifest in load_manifests().items():
            self.assertLessEqual(int(manifest["restart_limit"]), MAX_RESTART_LIMIT, service_id)
            self.assertLessEqual(
                int(manifest["resource_process_limit"]), MAX_PROCESS_LIMIT, service_id
            )
            self.assertLessEqual(
                int(manifest["resource_cpu_weight"]), MAX_CPU_WEIGHT, service_id
            )
            self.assertLessEqual(
                int(manifest["resource_io_weight"]), MAX_IO_WEIGHT, service_id
            )
            self.assertLessEqual(
                int(manifest["shutdown_timeout_ms"]), MAX_SHUTDOWN_TIMEOUT_MS, service_id
            )

    def test_ipc_hardening_constants_are_declared(self) -> None:
        ipc = (ROOT / "system" / "aether-system-core" / "src" / "ipc.rs").read_text(
            encoding="utf-8"
        )
        self.assertIn("MAX_IPC_REQUEST_BYTES: usize = 8 * 1024", ipc)
        self.assertIn("MAX_IPC_RESPONSE_BYTES: usize = 1024 * 1024", ipc)
        self.assertIn("from_mode(0o600)", ipc)

    def test_permission_audit_and_resource_docs_exist(self) -> None:
        self.assertTrue(
            (ROOT / "docs" / "architecture" / "system-core-hardening.md").is_file()
        )
        permission = (
            ROOT / "system" / "aether-system-core" / "src" / "permission.rs"
        ).read_text(encoding="utf-8")
        self.assertIn("ServiceControl", permission)
        self.assertIn("SystemControl", permission)
        self.assertIn("private local IPC", permission)
        self.assertIn("filesystem operations require private local IPC", permission)

        audit = (ROOT / "system" / "aether-system-core" / "src" / "audit.rs").read_text(
            encoding="utf-8"
        )
        self.assertIn("correlation_id", audit)
        self.assertIn("decision", audit)

    def test_filesystem_security_docs_and_tests_exist(self) -> None:
        for path in [
            ROOT / "docs" / "architecture" / "filesystem-service.md",
            ROOT / "docs" / "architecture" / "storage-service.md",
            ROOT / "docs" / "security" / "filesystem-security.md",
            ROOT / "docs" / "development" / "filesystem.md",
        ]:
            self.assertTrue(path.is_file(), path)

        path_security = (ROOT / "storage" / "aether-storage" / "src" / "path.rs").read_text(
            encoding="utf-8"
        )
        for marker in [
            "PathTraversal",
            "SymlinkEscape",
            "absolute paths are not accepted",
            "null bytes",
            "restricted by policy",
        ]:
            self.assertIn(marker, path_security)

        service = (ROOT / "storage" / "aether-storage" / "src" / "service.rs").read_text(
            encoding="utf-8"
        )
        self.assertIn("blocks_traversal_absolute_and_symlink_escape", service)
        self.assertIn("enforces_recursive_delete_limits", service)

    def test_filesystem_capability_catalog_is_complete(self) -> None:
        authorization = (
            ROOT / "storage" / "aether-storage" / "src" / "authorization.rs"
        ).read_text(encoding="utf-8")
        for capability in [
            "filesystem.read",
            "filesystem.write",
            "filesystem.create",
            "filesystem.rename",
            "filesystem.move",
            "filesystem.copy",
            "filesystem.delete",
            "filesystem.list",
            "filesystem.stat",
            "filesystem.search",
            "filesystem.watch",
            "filesystem.mount.read",
            "filesystem.storage.info",
        ]:
            self.assertIn(capability, authorization)
        self.assertIn("RiskLevel::Critical", authorization)


def load_manifests() -> dict[str, dict[str, str]]:
    manifests: dict[str, dict[str, str]] = {}
    for path in sorted((ROOT / "system" / "services.d").glob("*.aether-service")):
        values: dict[str, str] = {}
        for raw_line in path.read_text(encoding="utf-8").splitlines():
            line = raw_line.strip()
            if not line or line.startswith("#"):
                continue
            key, value = line.split("=", 1)
            values[key] = value
        manifests[values["service_id"]] = values
    return manifests


if __name__ == "__main__":
    unittest.main()
