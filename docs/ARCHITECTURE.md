# MechKeyMesh architecture

## Product direction

MechKeyMesh is a **multi-vendor keyboard platform**, not an AULA-specific application.

The **AULA F108 Pro is the first complete implementation**. It is intentionally used to drive the shared architecture with real hardware requirements instead of designing abstract interfaces that have never been tested against a device.

## Workspace structure

```text
Cargo workspace
│
├── crates/
│   ├── config/               desired-state domain model
│   ├── core/                 registry, lifecycle, serialization, diagnostics
│   ├── driver-api/           device/capability/driver contracts
│   ├── hid/                  HID discovery + transport abstraction
│   └── persistence/          SQLite storage + migrations
│
├── drivers/
│   └── aula-f108pro/         first device driver
│
└── apps/
    └── desktop/
        ├── frontend/
        └── src-tauri/
```

Future keyboards belong under `drivers/`. A driver may support any subset of standardized capabilities.

## Driver registry

The application does not own a hard-coded active keyboard driver. Drivers register with `DriverRegistry`.

```text
DriverRegistry
│
├── aula.f108pro.usb
├── future.rmk
├── future.vendor.x
└── ...
```

Every driver exposes a `DriverManifest` containing:

- stable driver ID
- vendor and device name
- VID/PID matchers
- usage page and optional interface constraints
- supported connection types
- currently implemented capabilities
- transport requirements

Discovery returns `DeviceCandidate` values. A candidate contains a stable per-enumeration instance identifier, such as the Windows HID path, so the manager can distinguish one physical device instance from another.

The registry rejects duplicate driver IDs and removes duplicate candidate reports from the same driver.

## Capability model

Examples:

```text
Keys
FnLayer
Macros
HardwareRgb
PerKeyRgb
LiveRgb
Music
Display
Clock
DeviceSettings
Battery
Encoder
Firmware
```

A driver MUST advertise only capabilities that are actually implemented. A known hardware feature is not automatically an implemented MechKeyMesh capability.

## First driver: AULA F108 Pro

Current identity:

```text
Driver ID   aula.f108pro.usb
VID         0C45
PID         800A
Usage Page  FF13
Connection  USB
Transport   HID Feature Report, 64-byte payload
```

Windows discovery is anchored on VID/PID plus Usage Page `FF13`. The actual discovered HID interface number is recorded in the candidate instead of assuming that one interface-number representation is portable across all HID backends.

## Device Manager 2.0

Only the device manager owns an open `DeviceSession`.

```text
Frontend
   |
   v
Tauri commands
   |
   v
DeviceManagerHandle
   |
   v
serialized command queue
   |
   v
active DeviceSession
   |
   v
KeyboardDriver
   |
   v
Transport
```

Lifecycle:

```text
Disconnected
     |
     v
Detecting
     |
     v
Opening
     |
     v
Initializing
     |
     v
Ready
     |
     v
Busy
```

Failure path:

```text
failure
   |
   v
Recovery
   |
   v
close stale session
   |
   v
Detect
   |
   v
Reconnect
```

Special non-ready states include:

- `Unsupported`: no registered driver can operate on the current platform
- `Ambiguous`: multiple supported keyboard instances exist and automatic selection is intentionally blocked

## Timeouts and cancellation

Every hardware operation receives an `OperationContext` with:

- a deadline
- a cooperative cancellation token

The manager enforces a command timeout and exposes the active operation through diagnostics. F108 protocol delays use the operation context instead of unconditional sleeps, so cancellation and timeout are observed between packets.

A timed-out or cancelled operation cannot continue indefinitely through the remaining transaction steps.

## Retry policy

R2 uses bounded retries only for opening a device after transient errors.

It does **not** automatically replay hardware configuration commands after a partial failure. A packet sequence may have partially changed device state, so automatic command replay is unsafe unless that specific protocol is proven idempotent.

## Device removal detection

While a session is Ready, the manager periodically asks the active driver whether the exact discovered device candidate is still present.

For the F108 Pro this is performed by HID rediscovery of the same instance path. MechKeyMesh does not send undocumented probe commands simply to test liveness.

If the instance disappears:

```text
Ready
  ↓
Recovery
  ↓
close stale handle
  ↓
Disconnected
  ↓
rediscovery
```

When the keyboard returns, the normal discovery/open/initialize path runs again.

## Duplicate-device protection

MechKeyMesh does not silently choose an arbitrary keyboard when more than one supported candidate is visible.

Until explicit multi-device selection is implemented, multiple candidates produce `Ambiguous` state. This protects against writing configuration to the wrong physical keyboard.

## Transport errors

Transport errors are classified into stable categories such as:

- NotFound
- Disconnected
- PermissionDenied
- Busy
- Timeout
- Cancelled
- Unsupported
- Protocol
- Transport
- DuplicateDevice
- Internal

The manager uses the classification to decide whether an error requires recovery or can leave the current device session open.

## Safety model

Three levels remain distinct:

```text
transport succeeded
device acknowledged
functional result verified
```

An ACK is not proof that a feature worked.

For high-risk features, packet generation and media conversion must be testable without access to physical hardware. LCD upload will not be enabled until its hard bounds are encoded and regression-tested.

## Ecosystem rule

> MechKeyMesh does not support a new keyboard by adding device-specific branches to the application. It supports a new keyboard by adding or extending a driver that implements standardized capabilities.


## Desired-state persistence

MechKeyMesh treats the local database as the source of truth for configuration that cannot be reliably reconstructed from the keyboard.

```text
SQLite desired state
        |
        v
configuration scope
(device model + profile + capability)
        |
        v
driver translation
        |
        v
keyboard
```

Configuration is never scoped merely to whichever keyboard happens to be connected at the moment.

A stable `device_model_id` is separate from a connection-specific `driver_id`. For example:

```text
device model   aula.f108pro
USB driver     aula.f108pro.usb
future 2.4G    can share aula.f108pro
```

This allows USB and future wireless drivers for the same physical keyboard model to share profiles and desired configuration.

### SQLite policy

The persistence layer uses:

- foreign keys enabled
- WAL journal mode
- `synchronous = FULL`
- bounded busy timeout
- transactional schema migrations
- `PRAGMA quick_check` integrity validation

Schema changes are versioned. A newer database is refused by an older MechKeyMesh build instead of being silently modified.

### Current schemas

```text
schema 1  devices, profiles, lighting, device_settings, application_rules
schema 2  key_mappings, layer_mappings, macros, macro_events
schema 3  per_key_lighting, live_lighting, music_presets, lcd_projects
```

The tables exist before their higher-level feature editors are implemented so later runs can add functionality without repeatedly redesigning persistence ownership.

### Desired lighting behavior

The first end-to-end persisted feature is onboard lighting.

When the user saves a lighting configuration:

1. MechKeyMesh writes the desired state to SQLite.
2. If the keyboard is Ready, the configuration is then sent through the device manager.
3. If the keyboard is unavailable, the desired state remains safely stored.
4. On the next application start, the editor reloads the saved desired state.

Automatic reconciliation of all desired state after reconnect will be expanded in later feature runs rather than guessed prematurely.
