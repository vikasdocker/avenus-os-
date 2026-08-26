# Build System

Aether OS uses separate build systems for separate engineering domains:

| Domain | Build System | Entry Point |
| --- | --- | --- |
| Bootable Linux image | Buildroot external tree | `scripts/build/build.sh` |
| Rust services and SDK | Cargo workspace | `Cargo.toml` |
| C system utilities | CMake | `CMakeLists.txt` |
| Python brain package | Python standard tooling | `pyproject.toml` |
| Qt/QML shell | CMake optional target | `ui/shell/CMakeLists.txt` |
| ISO assembly | Shell scripts | `scripts/iso/` |
| Linux kernel | Kernel build helper scripts | `kernel/scripts/` |

The root scripts orchestrate common tasks:

```bash
bash scripts/build.sh
bash scripts/test.sh
bash scripts/lint.sh
bash scripts/format.sh
bash scripts/clean.sh
```

The default build excludes the Qt shell because CI and headless build agents may not
have Qt installed. Enable it explicitly on machines with Qt 6:

```bash
cmake -S . -B build/cmake/ui -DAETHER_BUILD_UI=ON
cmake --build build/cmake/ui
```
