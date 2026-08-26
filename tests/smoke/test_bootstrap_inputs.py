from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[2]


class BootstrapSmokeTests(unittest.TestCase):
    def test_iso_scripts_exist(self) -> None:
        self.assertTrue((ROOT / "scripts" / "build" / "bootstrap.sh").is_file())
        self.assertTrue((ROOT / "scripts" / "build" / "build.sh").is_file())
        self.assertTrue((ROOT / "scripts" / "run" / "qemu-buildroot.sh").is_file())
        self.assertTrue((ROOT / "scripts" / "iso" / "build-initramfs.sh").is_file())
        self.assertTrue((ROOT / "scripts" / "iso" / "build-iso.sh").is_file())
        self.assertTrue((ROOT / "scripts" / "run" / "qemu.sh").is_file())

    def test_buildroot_external_tree_exists(self) -> None:
        external = ROOT / "infra" / "buildroot" / "external"
        self.assertTrue((external / "external.desc").is_file())
        self.assertTrue((external / "external.mk").is_file())
        self.assertTrue((external / "Config.in").is_file())
        self.assertTrue((external / "configs" / "aether_x86_64_qemu_defconfig").is_file())

    def test_kernel_config_contains_initramfs_support(self) -> None:
        config = (ROOT / "kernel" / "configs" / "aether-x86_64.config").read_text(
            encoding="utf-8"
        )
        self.assertIn("CONFIG_BLK_DEV_INITRD=y", config)
        self.assertIn("CONFIG_RD_GZIP=y", config)
        self.assertIn("CONFIG_DEVTMPFS=y", config)

    def test_boot_services_are_declared(self) -> None:
        service_dir = ROOT / "system" / "services.d"
        services = sorted(path.name for path in service_dir.glob("*.service"))
        self.assertEqual(services, ["aether-agentd.service", "aether-healthd.service"])


if __name__ == "__main__":
    unittest.main()
