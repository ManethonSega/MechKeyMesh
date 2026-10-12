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
- F108 Pro 2.4 GHz discovery through dongle `05AC:024F`, interface 3, with a connection-specific capability matrix and battery reporting
- host-generated layered live RGB on USB through the volatile `04 20` preview path, with a one-frame latest-value queue
- Windows output-loopback music lighting with FFT bands, smoothing, falloff, frequency weights and five render modes
- 20 mapped onboard lighting effects with effect-specific parameter metadata; physical verification remains explicitly unverified/deferred
- independent Normal/Fn desired keymap persistence and complete 576-byte F108 table generation
- persistent per-key RGB desired state using physical key IDs and layout LED indices
- universal macro model, transactional persistence, timeline editor and hardware-free simulation
- preflighted F108 macro-bank serializer, upload queue and stable macro key references; physical verification remains pending
- complete F108 function-settings desired state for locks, Fn behavior, sleep and key response; physical verification remains pending
- verified software encoders for keyboard keys, modifiers, shortcuts, media, selected mouse actions and selected Windows/system actions
- strict lighting and remap parameter validation
- runtime diagnostics
- SQLite desired-state database with versioned migrations
- stable device-model/profile/capability configuration scope
- universal data-driven keyboard layout and KeyAction engine
- brand-consistent dark and light application themes
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

`v0.18.0-alpha.1`, **R17 profiles and automatic switching in validation**. Profiles clone and combine mappings, Fn mappings, macros, onboard and per-key RGB, live effects, music, device settings and an optional validated LCD asset. A native Windows foreground-process monitor applies executable rules, while per-subsystem state fingerprints prevent unchanged systems from generating USB traffic. Physical switching validation remains pending because no keyboard is available in this environment.

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
- https://github.com/kidush/omarchy-aula-f108pro
- https://github.com/ThePrimeDev/SignalRgb_AulaF108Pro
- https://github.com/Punkster81/AULA-F108-Driver

See [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md).
