# MechKeyMesh architecture

## Product direction

MechKeyMesh is a **multi-vendor keyboard platform**, not an AULA-specific application.

The **AULA F108 Pro is the first complete implementation**. It is intentionally used to drive the shared architecture with real hardware requirements instead of designing abstract interfaces that have never been tested against a device.

## Workspace structure

```text
Cargo workspace
│
├── crates/
│   ├── core/                 registry, lifecycle, serialization, diagnostics
│   ├── driver-api/           device/capability/driver contracts
│   └── hid/                  HID discovery + transport abstraction
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
