# Shell Domain

The shell domain owns user-session behavior, command surfaces, and shell policy that sits
above the graphical implementation. The current Qt/QML implementation lives in
`ui/shell`; this folder records shell-domain contracts that apply regardless of the
rendering technology used by future desktop, tablet, mobile, vehicle, or robot profiles.

## Phase 0.4 Contract

- The shell must expose local system status.
- The shell must accept a text command.
- The shell must report whether local control is ready.
- The shell must not contain security policy decisions.
- The shell must preserve a path to diagnostics and recovery.

## Build Relationship

The build target for the first graphical shell is:

```bash
cmake -S . -B build/cmake/ui -DAETHER_BUILD_UI=ON
cmake --build build/cmake/ui
```

