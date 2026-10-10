# MechKeyMesh

**Universal Keyboard Studio**

MechKeyMesh is a vendor-neutral Windows keyboard configuration platform. The first supported device is the **AULA F108 Pro**, but the architecture is deliberately driver-based so other brands, sizes, layouts, displays and encoders can be added without rewriting the application.

## Current development status

`v0.1.0-alpha.1` starts the Phase 1 foundation:

- Wails desktop shell and MechKeyMesh UI foundation
- central device manager and serialized hardware command queue
- explicit device lifecycle states and reconnect loop
- first AULA F108 Pro USB driver (VID `0C45`, PID `800A`)
- safe built-in lighting control for the 20 known hardware effects
- diagnostics/log buffer exposed to the UI
- platform stub so the core can be tested outside Windows
- protocol tests for lighting payload validation

The first alpha intentionally **does not** write LCD media, macros, firmware or unverified settings. Those features are added only after their safety constraints and packet formats are covered by tests.

## Development priorities

1. Reliability
2. Hardware safety
3. Complete OEM replacement
4. Usability
5. Advanced functionality

See [`docs/MANIFESTO.md`](docs/MANIFESTO.md) for the product blueprint and [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for the current implementation architecture.

## Build stack

- Go backend
- Wails v2 desktop shell
- HTML/CSS/JavaScript frontend initially, with the UI/backend boundary kept framework-agnostic
- Windows native HID API for the F108 Pro

### Windows development

Install Go, Node.js and the stable Wails v2 CLI, then run:

```powershell
go install github.com/wailsapp/wails/v2/cmd/wails@v2.16.0
wails doctor
wails dev
```

### Backend tests

```powershell
go test ./internal/...
```

## Hardware safety

Two project rules are permanent:

> No user action may generate an LCD write outside a device's documented user-media region.

> A device ACK proves transport, not functional correctness. Unverified commands do not enter stable releases.

## Upstream research

The AULA F108 Pro driver work builds on documented, MIT-licensed community research from:

- https://github.com/parsiya/f108-pro
- https://github.com/Punkster81/AULA-F108-Driver

See [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md).
