from pathlib import Path
import os
import shutil
import subprocess
import threading
import time
import unittest


ROOT = Path(__file__).resolve().parents[2]


class QemuBootTests(unittest.TestCase):
    def test_buildroot_image_boots_to_shell_and_shuts_down(self) -> None:
        if os.environ.get("AETHER_BOOT_TEST") != "1":
            self.skipTest("set AETHER_BOOT_TEST=1 or run scripts/test-boot.sh")

        qemu = _which("qemu-system-x86_64")
        self.assertIsNotNone(qemu, "qemu-system-x86_64 is required")

        images_dir = Path(os.environ.get("AETHER_IMAGES_DIR", ROOT / "artifacts/buildroot/output/images"))
        kernel = Path(os.environ.get("AETHER_KERNEL_IMAGE", images_dir / "bzImage"))
        rootfs = Path(os.environ.get("AETHER_ROOTFS_IMAGE", images_dir / "rootfs.ext2"))
        self.assertTrue(kernel.is_file(), f"kernel image missing: {kernel}")
        self.assertTrue(rootfs.is_file(), f"root filesystem missing: {rootfs}")

        command = [
            qemu,
            "-machine",
            "q35",
            "-cpu",
            "max",
            "-smp",
            "2",
            "-m",
            "512",
            "-kernel",
            str(kernel),
            "-drive",
            f"file={rootfs},if=virtio,format=raw",
            "-append",
            "console=ttyS0 root=/dev/vda rw init=/sbin/aether-init panic=-1",
            "-netdev",
            "user,id=net0",
            "-device",
            "virtio-net-pci,netdev=net0",
            "-display",
            "none",
            "-serial",
            "stdio",
            "-monitor",
            "none",
            "-no-reboot",
        ]

        process = subprocess.Popen(
            command,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            bufsize=1,
        )
        output: list[str] = []
        reader = threading.Thread(target=_read_output, args=(process, output), daemon=True)
        reader.start()

        try:
            _wait_for(output, "Linux version", 45)
            _wait_for(output, "AETHER_INIT_STARTED", 30)
            _wait_for(output, "AETHER_SYSTEM_CORE_READY", 30)
            _wait_for(output, "AETHER_FILESYSTEM_READY", 30)
            _wait_for(output, "AETHER_CORE_READY", 30)
            _wait_for(output, "AETHER_SHELL_READY", 30)
            _wait_for(output, "AETHER_NETWORK_READY", 30)
            _write(process, "ifconfig eth0\n")
            _wait_for(output, "eth0", 10)
            _write(process, "ping -c 1 -W 3 10.0.2.2\n")
            _wait_for_any(output, ["1 packets received", "1 packets transmitted, 1 packets received"], 15)
            _write(process, "aetherctl fs health\n")
            _wait_for(output, "\"component\":\"aether-filesystemd\"", 10)
            _write(process, "aetherctl fs stat tmp\n")
            _wait_for(output, "type=directory", 10)
            _write(process, "aetherctl fs search tmp aether\n")
            _wait_for(output, "path|type|size_bytes|modified_ms", 10)
            _write(process, "aetherctl fs stat ../../etc/passwd\n")
            _wait_for_any(output, ["PathTraversal", "permission denied"], 10)
            _write(process, "aether-shutdown\n")
            process.wait(timeout=30)
            self.assertEqual(process.returncode, 0, "\n".join(output[-200:]))
        finally:
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()


def _which(command: str) -> str | None:
    return shutil.which(command)


def _read_output(process: subprocess.Popen[str], output: list[str]) -> None:
    assert process.stdout is not None
    for line in process.stdout:
        output.append(line.rstrip())


def _write(process: subprocess.Popen[str], value: str) -> None:
    assert process.stdin is not None
    process.stdin.write(value)
    process.stdin.flush()


def _wait_for(output: list[str], needle: str, timeout_seconds: int) -> None:
    deadline = time.monotonic() + timeout_seconds
    while time.monotonic() < deadline:
        if any(needle in line for line in output):
            return
        time.sleep(0.1)
    tail = "\n".join(output[-200:])
    raise AssertionError(f"timed out waiting for {needle!r}\n{tail}")


def _wait_for_any(output: list[str], needles: list[str], timeout_seconds: int) -> None:
    deadline = time.monotonic() + timeout_seconds
    while time.monotonic() < deadline:
        if any(any(needle in line for needle in needles) for line in output):
            return
        time.sleep(0.1)
    tail = "\n".join(output[-200:])
    raise AssertionError(f"timed out waiting for any of {needles!r}\n{tail}")


if __name__ == "__main__":
    unittest.main()
