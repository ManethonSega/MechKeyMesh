# R1: Rust/Tauri platform foundation + AULA F108 Pro first driver

## Status

**COMPLETE**

R1 is the software-foundation run. It establishes the Rust/Tauri architecture and proves it through automated Linux and Windows validation. Physical AULA F108 Pro acceptance testing is intentionally deferred because the hardware is not currently available to the project owner. That deferred hardware validation is recorded below and is not being misrepresented as passed.

## Goal

Replace the short-lived Go/Wails prototype before more functionality depends on it, while keeping the **AULA F108 Pro** as the first real device implementation.

## Implemented

- [x] Rust Cargo workspace
- [x] Tauri 2 desktop shell
- [x] vendor-neutral driver API
- [x] explicit Device / Connection / Capability / Driver / DeviceSession concepts
- [x] HID transport abstraction
- [x] Windows-native HID backend through `hidapi`
- [x] serialized device manager
- [x] reconnect/recovery loop
- [x] bounded diagnostics/log buffer
- [x] AULA F108 Pro driver crate
- [x] first registered driver ID: `aula.f108pro.usb`
- [x] F108 Pro USB identity: VID `0C45`, PID `800A`
- [x] F108 Pro configuration Usage Page `FF13`
- [x] initial F108 onboard lighting protocol
- [x] 20 known onboard lighting modes
- [x] lighting bounds validation
- [x] required readback/ACK validation
- [x] capability reporting restricted to actually implemented capability
- [x] Rust formatting checks
- [x] Rust unit/protocol tests
- [x] Linux Clippy
- [x] Windows workspace tests
- [x] Windows Clippy
- [x] Windows release executable build and existence verification
- [x] old Go/Wails prototype preserved in `archive/go-wails-prototype`
- [x] Rust/Tauri migration merged into `main`

## R1 automated exit criterion

The agreed R1 exit criterion is:

```text
cargo test --workspace
cargo clippy
Windows desktop build
```

All passed in GitHub Actions workflow run **38077041747** on commit `aa0663817abb1db27c42ca86e29400b7305970ec` before merge.

The migration was squash-merged into `main` as commit:

```text
ddf6f9ea061f888e381509d3f7991a0844c5e5f3
```

## 12-step run record

| Step | Result | Record |
|---|---|---|
| 1. Define run | Complete | Rust/Tauri foundation scope fixed |
| 2. Research/map protocol | Complete for R1 scope | F108 USB identity, FF13 configuration HID, 64-byte feature reports, lighting transaction and 35 ms command delay mapped |
| 3. Generic model | Complete | Core, driver API and HID transport separated |
| 4. F108 translation | Complete | First F108 driver and onboard-lighting path implemented |
| 5. Regression tests | Complete | packet bounds, ACK validation, manager serialization and logging covered |
| 6. CI | Complete | Linux and Windows test/lint jobs green |
| 7. Windows test build | Complete | release EXE compiled and verified in CI |
| 8. Physical F108 test | Deferred | owner cannot access/test physical keyboard now |
| 9. Record results | Complete | hardware validation explicitly recorded as not executed |
| 10. Fix regressions | Complete for observed software/CI regressions | Tauri icon/build issue, formatting, capability over-reporting and ACK validation corrected |
| 11. Mark R1 complete | Complete | R1 closed as software-foundation run |
| 12. Start next run | Not part of R1 completion | R2 starts only on explicit instruction |

## Deferred physical acceptance

These tests remain **not executed**, not failed:

- [ ] start application with keyboard disconnected
- [ ] connect F108 and reach Ready
- [ ] start application with F108 already connected
- [ ] unplug without crash/freeze
- [ ] reconnect without application restart
- [ ] apply representative onboard lighting modes
- [ ] verify brightness
- [ ] verify speed
- [ ] verify direction
- [ ] verify colorful/rainbow behavior
- [ ] verify repeated reconnect cycles
- [ ] observe behavior when OEM software also has the device open

They should be run at the next available hardware-validation window. Any defect found becomes a regression issue for the earliest active run and must be fixed before a public hardware-supported release.

## Intentionally not enabled in R1

- LCD writes
- key remapping
- Fn remapping
- macros
- persistent device settings
- firmware flashing
- 2.4 GHz configuration
- per-key RGB
- live RGB
- music lighting

These remain gated for later runs.

## R1 conclusion

The **Rust/Tauri foundation is now the authoritative MechKeyMesh implementation on `main`**. No new development should continue on the archived Go/Wails prototype.
