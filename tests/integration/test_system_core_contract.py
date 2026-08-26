from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[2]

REQUIRED_MANIFEST_FIELDS = {
    "schema_version",
    "service_id",
    "name",
    "version",
    "description",
    "service_type",
    "dependencies",
    "startup_priority",
    "restart_policy",
    "restart_limit",
    "restart_backoff_ms",
    "health_check",
    "security_identity",
    "requires_root",
    "sandbox_profile",
    "permission_profile",
    "ipc_access",
    "resource_cpu_weight",
    "resource_memory_max_kib",
    "resource_process_limit",
    "resource_io_weight",
    "shutdown_timeout_ms",
}


class SystemCoreContractTests(unittest.TestCase):
    def test_workspace_contains_system_core_and_aetherctl(self) -> None:
        cargo = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
        self.assertIn("system/aether-system-core", cargo)
        self.assertIn("tools/aetherctl", cargo)

    def test_system_core_modules_exist(self) -> None:
        module_dir = ROOT / "system" / "aether-system-core" / "src"
        for module in [
            "config.rs",
            "dependency.rs",
            "event.rs",
            "health.rs",
            "ipc.rs",
            "lifecycle.rs",
            "logging.rs",
            "manager.rs",
            "manifest.rs",
            "metrics.rs",
            "recovery.rs",
            "registry.rs",
            "shutdown.rs",
            "state.rs",
            "types.rs",
        ]:
            self.assertTrue((module_dir / module).is_file(), module)

    def test_default_manifests_are_complete(self) -> None:
        manifests = load_manifest_dir(ROOT / "system" / "services.d")
        self.assertEqual(
            sorted(manifests),
            ["aether.config", "aether.core", "aether.filesystem", "aether.ipc", "aether.logging"],
        )
        for service_id, values in manifests.items():
            missing = REQUIRED_MANIFEST_FIELDS - values.keys()
            self.assertEqual(missing, set(), service_id)
            self.assertEqual(values["schema_version"], "1")
            self.assertGreater(int(values["startup_priority"]), 0)
            self.assertGreater(int(values["shutdown_timeout_ms"]), 0)
            self.assertIn(values["requires_root"], {"true", "false"})
            self.assertIn(values["ipc_access"], {"local-private"})
            self.assertGreater(int(values["resource_process_limit"]), 0)
            self.assertGreater(int(values["resource_io_weight"]), 0)

    def test_dependency_order_is_dependency_first(self) -> None:
        manifests = load_manifest_dir(ROOT / "system" / "services.d")
        order = resolve_order(manifests)
        self.assertLess(order.index("aether.ipc"), order.index("aether.logging"))
        self.assertLess(order.index("aether.logging"), order.index("aether.config"))
        self.assertLess(order.index("aether.config"), order.index("aether.filesystem"))
        self.assertLess(order.index("aether.config"), order.index("aether.core"))

    def test_missing_and_circular_dependencies_are_detectable(self) -> None:
        missing = {
            "aether.alpha": {
                "dependencies": "aether.missing",
                "startup_priority": "10",
            }
        }
        with self.assertRaises(ValueError):
            resolve_order(missing)

        cycle = {
            "aether.alpha": {"dependencies": "aether.beta", "startup_priority": "10"},
            "aether.beta": {"dependencies": "aether.alpha", "startup_priority": "20"},
        }
        with self.assertRaises(ValueError):
            resolve_order(cycle)

    def test_buildroot_selects_system_core_and_installs_manifests(self) -> None:
        defconfig = (
            ROOT
            / "infra"
            / "buildroot"
            / "external"
            / "configs"
            / "aether_x86_64_qemu_defconfig"
        ).read_text(encoding="utf-8")
        self.assertIn("BR2_PACKAGE_AETHER_SYSTEM_CORE=y", defconfig)
        package = (
            ROOT
            / "infra"
            / "buildroot"
            / "external"
            / "package"
            / "aether-system-core"
            / "aether-system-core.mk"
        ).read_text(encoding="utf-8")
        self.assertIn("-p aether-storage", package)
        self.assertIn("aether-filesystemd", package)

        overlay_dir = (
            ROOT
            / "infra"
            / "buildroot"
            / "external"
            / "overlays"
            / "rootfs"
            / "etc"
            / "aether"
            / "services.d"
        )
        self.assertTrue((overlay_dir / "aether-core.aether-service").is_file())
        self.assertTrue((overlay_dir / "aether-filesystem.aether-service").is_file())

    def test_init_prefers_system_core_and_cli_uses_ipc(self) -> None:
        init = (
            ROOT
            / "infra"
            / "buildroot"
            / "external"
            / "overlays"
            / "rootfs"
            / "sbin"
            / "aether-init"
        ).read_text(encoding="utf-8")
        self.assertIn("/usr/sbin/aether-system-core", init)
        self.assertIn("aetherctl --socket", init)
        self.assertIn("--audit-log /var/log/aether/aether-audit.log", init)
        self.assertIn("--filesystem-socket /run/aether/ipc/aether-filesystemd.sock", init)

        cli = (ROOT / "tools" / "aetherctl" / "src" / "main.rs").read_text(
            encoding="utf-8"
        )
        self.assertIn("send_request", cli)
        self.assertNotIn("ServiceManager::new", cli)
        self.assertIn("system audit", cli)
        self.assertIn("fs storage", cli)

    def test_phase_1_4_hardening_modules_are_present(self) -> None:
        lib = (ROOT / "system" / "aether-system-core" / "src" / "lib.rs").read_text(
            encoding="utf-8"
        )
        for module in ["audit", "permission", "resource"]:
            self.assertIn(f"pub mod {module};", lib)

        ipc = (ROOT / "system" / "aether-system-core" / "src" / "ipc.rs").read_text(
            encoding="utf-8"
        )
        self.assertIn("MAX_IPC_REQUEST_BYTES", ipc)
        self.assertIn("from_mode(0o600)", ipc)

        manager = (
            ROOT / "system" / "aether-system-core" / "src" / "manager.rs"
        ).read_text(encoding="utf-8")
        self.assertIn("evaluate_request", manager)
        self.assertIn("AuditEntry::new", manager)
        self.assertIn("validate_manifest", manager)

    def test_phase_1_5_filesystem_contract_is_present(self) -> None:
        manifest = load_manifest_dir(ROOT / "system" / "services.d")["aether.filesystem"]
        self.assertEqual(manifest["service_type"], "process")
        self.assertEqual(manifest["requires_root"], "true")
        self.assertEqual(manifest["permission_profile"], "system-internal")
        self.assertIn("filesystem.read", manifest["capabilities"])
        self.assertIn("filesystem.storage.info", manifest["capabilities"])

        storage = (ROOT / "storage" / "aether-storage" / "src" / "lib.rs").read_text(
            encoding="utf-8"
        )
        for module in [
            "authorization",
            "path",
            "metadata",
            "search",
            "storage_info",
            "watch",
            "service",
        ]:
            self.assertIn(f"pub mod {module};", storage)

        permission = (
            ROOT / "system" / "aether-system-core" / "src" / "permission.rs"
        ).read_text(encoding="utf-8")
        self.assertIn("FilesystemRead", permission)
        self.assertIn("CapabilityRegistry", permission)

        ipc = (ROOT / "system" / "aether-system-core" / "src" / "ipc.rs").read_text(
            encoding="utf-8"
        )
        self.assertIn("FileSystemRequest", ipc)
        self.assertIn("send_filesystem_request", ipc)


def load_manifest_dir(path: Path) -> dict[str, dict[str, str]]:
    manifests: dict[str, dict[str, str]] = {}
    for manifest_path in sorted(path.glob("*.aether-service")):
        values: dict[str, str] = {}
        for raw_line in manifest_path.read_text(encoding="utf-8").splitlines():
            line = raw_line.strip()
            if not line or line.startswith("#"):
                continue
            key, value = line.split("=", 1)
            values[key] = value
        manifests[values["service_id"]] = values
    return manifests


def resolve_order(manifests: dict[str, dict[str, str]]) -> list[str]:
    order: list[str] = []
    states: dict[str, str] = {}

    def visit(service_id: str, stack: list[str]) -> None:
        state = states.get(service_id)
        if state == "visited":
            return
        if state == "visiting":
            raise ValueError("cycle: " + " -> ".join(stack + [service_id]))
        if service_id not in manifests:
            raise ValueError(f"missing dependency: {service_id}")

        states[service_id] = "visiting"
        dependencies = [
            dependency.strip()
            for dependency in manifests[service_id].get("dependencies", "").split(",")
            if dependency.strip()
        ]
        for dependency in sorted(
            dependencies,
            key=lambda item: (int(manifests[item]["startup_priority"]), item)
            if item in manifests
            else (0, item),
        ):
            visit(dependency, stack + [service_id])
        states[service_id] = "visited"
        order.append(service_id)

    for service_id in sorted(
        manifests,
        key=lambda item: (int(manifests[item]["startup_priority"]), item),
    ):
        visit(service_id, [])
    return order


if __name__ == "__main__":
    unittest.main()
