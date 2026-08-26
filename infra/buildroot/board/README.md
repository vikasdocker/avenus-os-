# Board Policy

Board-specific files are currently stored inside the Buildroot external tree:

```text
infra/buildroot/external/board/aether/x86_64/
```

Future physical x86_64 PC, laptop, AI workstation, and ARM64 targets must add separate
board directories instead of weakening the QEMU board contract.

