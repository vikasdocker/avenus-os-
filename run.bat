@echo off
rem Boot Aether OS in QEMU on Windows
rem Usage:
rem   run.bat              serial console (default)
rem   run.bat --window     graphical window with serial console
rem   run.bat --smoke      headless smoke test
rem
rem Root cause fix: virtio-gpu-pci requires the virtio_gpu kernel module
rem which is NOT built into the stock Debian kernel and NOT in our initramfs.
rem Without it, the kernel has no framebuffer device, console=tty0 has nowhere
rem to output, and the boot appears to hang after BIOS.
rem Fix: use -vga std (standard VGA, always works without kernel modules).

set QEMU=C:\Program Files\qemu\qemu-system-x86_64.exe
set KERNEL=%~dp0build\vmlinuz
set INITRD=%~dp0build\initramfs.cpio.gz
set APPEND=console=ttyS0 console=tty0 quiet tsc=unstable panic=-1

if not exist "%KERNEL%" (
    echo ERROR: kernel not found at %KERNEL%
    echo Run scripts/iso/build-initramfs.sh in Docker first
    exit /b 1
)
if not exist "%INITRD%" (
    echo ERROR: initramfs not found at %INITRD%
    echo Run scripts/iso/build-initramfs.sh in Docker first
    exit /b 1
)

if "%1"=="--window" (
    echo Starting Aether OS in graphical window...
    "%QEMU%" -m 512M -display sdl -vga std -netdev user,id=n0,hostfwd=tcp::14748-:4748,hostfwd=tcp::14747-:4747 -device virtio-net-pci,netdev=n0 -no-reboot -kernel "%KERNEL%" -initrd "%INITRD%" -append "%APPEND%"
) else if "%1"=="--smoke" (
    echo Running smoke test...
    "%QEMU%" -m 512M -nographic -no-reboot -kernel "%KERNEL%" -initrd "%INITRD%" -append "console=ttyS0 panic=-1"
) else (
    echo Starting Aether OS serial console...
    "%QEMU%" -m 512M -nographic -no-reboot -netdev user,id=n0,hostfwd=tcp::14748-:4748,hostfwd=tcp::14747-:4747 -device virtio-net-pci,netdev=n0 -kernel "%KERNEL%" -initrd "%INITRD%" -append "console=ttyS0 panic=-1"
)
