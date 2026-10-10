# MechKeyMesh

**Universal Keyboard Studio**

MechKeyMesh is being built as a **large multi-vendor keyboard platform**. Keyboard-specific behavior lives in independent drivers behind a shared capability model, layout engine and device lifecycle.

## First implementation: AULA F108 Pro

The **AULA F108 Pro is the first complete device implementation and hardware validation target**.

That means we are not building a generic shell and postponing real hardware work. The platform architecture is vendor-neutral from day one, while development proceeds vertically through the F108 Pro until it can replace the OEM application reliably.

Current Rust implementation:

- Tauri 2 desktop shell
- Rust Cargo workspace
- vendor-neutral driver API
- serialized device manager and reconnect/recovery loop
- HID transport abstraction
- first driver crate: `mechkeymesh-driver-aula-f108pro`
- wired F108 Pro discovery, VID `0C45`, PID `800A`, Usage Page `FF13`
- 20 verified onboard lighting effects
- strict lighting parameter validation
- runtime diagnostics
- SQLite desired-state database with versioned migrations
- stable device-model/profile/capability configuration scope
- hardware-risk features gated until their protocol safety tests exist

## Architecture

```text
MechKeyMesh Desktop
        |
        v
MechKeyMesh Core
        |
        +--> Profiles / layouts / shared services
        |
        v
Driver API
        |
        +--> AULA F108 Pro        <-- first implementation
        +--> future AULA models
        +--> future Epomaker
        +--> future Ajazz
        +--> future vendors
        |
        v
Transport abstraction
        |
        +--> USB HID
        +--> future BLE / vendor transports
```

A new keyboard should be supported by adding a driver that declares and implements capabilities. Device-specific protocol code should not leak into the desktop UI or shared core.

## Development priorities

1. Reliability
2. Hardware safety
3. Complete AULA F108 Pro replacement
4. Multi-vendor driver ergonomics
5. Usability
6. Advanced functionality

The full product blueprint lives in [`docs/MANIFESTO.md`](docs/MANIFESTO.md).

## Current version

`v0.4.0-alpha.1`, **R3 Persistence and Desired State complete**. SQLite is now the authoritative local source of truth for desired configuration, with versioned migrations and stable device-model/profile/capability scoping.

The previous Go/Wails prototype is preserved in the branch:

`archive/go-wails-prototype`

## Build prerequisites

- Rust 1.90+
- Windows for the current F108 Pro HID implementation
- WebView2 runtime for the Tauri desktop shell

### Run core tests

```powershell
cargo test --workspace --exclude mechkeymesh-desktop
```

### Build the Windows desktop executable

```powershell
cargo build -p mechkeymesh-desktop --release
```

## Hardware safety rules

> No user action may generate an LCD write outside a device's documented user-media region.

> A device ACK proves transport, not functional correctness. Unverified commands do not enter stable releases.

## Research sources

The F108 Pro implementation builds on our OEM analysis and MIT-licensed community research:

- https://github.com/parsiya/f108-pro
- https://github.com/Punkster81/AULA-F108-Driver

See [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md).
