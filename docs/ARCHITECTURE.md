# MechKeyMesh architecture

## Product direction

MechKeyMesh is a **multi-vendor keyboard platform**, not an AULA-specific application.

The **AULA F108 Pro is the first complete implementation**. It is intentionally used to drive the shared architecture with real hardware requirements instead of designing abstract interfaces that have never been tested against a device.

## Workspace structure

```text
Cargo workspace
│
├── crates/
│   ├── core/                 lifecycle, serialization, shared state
│   ├── driver-api/           stable device/capability contracts
│   └── hid/                  transport abstraction
│
├── drivers/
│   └── aula-f108pro/         first device driver
│
└── apps/
    └── desktop/
        ├── frontend/         static TypeScript/JS-ready UI
        └── src-tauri/        Tauri 2 application shell
```

Future keyboards belong under `drivers/`. A driver may support any subset of standardized capabilities.

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

The desktop application does not assume that every keyboard supports every capability. Eventually the UI should be generated/enabled from the active device's declared capabilities.

## First driver: AULA F108 Pro

Current identity:

```text
VID        0C45
PID        800A
Usage Page FF13
Connection USB
```

The first safe vertical slice implements:

- HID discovery/open
- central command serialization
- reconnect/recovery
- onboard lighting
- diagnostics

The next F108 Pro work proceeds through remapping, Fn layer, per-key RGB, macros, device settings and finally the safety-critical LCD path.

## Central device manager

Only the device manager owns an active device session.

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
single worker thread / serialized queue
   |
   v
DeviceSession
   |
   v
Driver
   |
   v
Transport
```

This prevents future RGB, display, macros and settings modules from writing to HID concurrently.

Lifecycle:

```text
Disconnected -> Detecting -> Opening -> Ready
                                 ^        |
                                 |        v
                              Recovery <- Busy
```

A failed hardware command invalidates the session and returns discovery to a known state rather than keeping stale handles.

## Driver boundary

`mechkeymesh-driver-api` defines vendor-neutral identity, capabilities and operations.

Device-specific knowledge belongs in the driver:

- VID/PID and interface selection
- packet formats
- transaction order
- device limits
- mode IDs
- firmware quirks

Shared code must not contain AULA magic values.

## Transport boundary

`mechkeymesh-hid` handles HID access. The first backend uses the Rust `hidapi` crate with its Windows-native backend.

Future transports may include:

- Linux hidraw
- macOS HID
- Bluetooth LE GATT
- vendor-specific USB bulk/interrupt paths
- serial or network transports

Drivers should depend on transport contracts rather than desktop-framework APIs.

## Safety model

Three levels are distinct:

```text
transport succeeded
device acknowledged
functional result verified
```

An ACK is not proof that a feature worked.

For high-risk features, packet generation and media conversion must be testable without access to physical hardware. LCD upload will not be enabled until its hard bounds are encoded and regression-tested.

## Ecosystem rule

> MechKeyMesh does not support a new keyboard by adding device-specific branches to the application. It supports a new keyboard by adding or extending a driver that implements standardized capabilities.
