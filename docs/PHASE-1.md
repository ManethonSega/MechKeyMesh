# Phase 1: Rust/Tauri platform foundation + AULA F108 Pro vertical slice

## Goal

Prove the new multi-vendor architecture against the **AULA F108 Pro**, our first implementation, without exposing risky hardware operations prematurely.

## Implemented

- [x] Cargo workspace
- [x] Tauri 2 desktop shell
- [x] vendor-neutral driver API
- [x] capability declarations
- [x] HID transport abstraction
- [x] Windows-native HID backend
- [x] serialized device manager
- [x] reconnect/recovery loop
- [x] diagnostics/log buffer
- [x] AULA F108 Pro driver crate
- [x] F108 Pro wired identity and Usage Page selection
- [x] 20 onboard lighting modes
- [x] lighting bounds validation
- [x] unit tests for F108 lighting payloads

## Intentionally not enabled yet

- LCD writes
- remapping
- macros
- device settings
- firmware flashing
- 2.4 GHz configuration
- live RGB streaming

These remain gated because the first release criterion is correctness and recoverability, not feature count.

## Hardware validation checklist

On the target Windows PC with the AULA F108 Pro:

- [ ] application starts with keyboard disconnected
- [ ] connecting the keyboard moves state to Ready
- [ ] unplugging does not crash or freeze the app
- [ ] reconnecting returns to Ready without app restart
- [ ] all 20 onboard lighting modes apply
- [ ] brightness is constrained to 0..5
- [ ] speed is constrained to 0..5
- [ ] invalid mode values never reach USB
- [ ] competing OEM software produces a readable access error
- [ ] repeated reconnect cycles remain stable

## Exit criterion

> The Rust/Tauri MechKeyMesh foundation can keep running through repeated AULA F108 Pro disconnect/reconnect cycles and apply verified onboard lighting without crashes, stale-device state or unsafe commands.

Only after this passes do we move to the next F108 Pro subsystem.
