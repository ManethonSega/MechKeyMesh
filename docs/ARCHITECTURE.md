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
│   ├── layout/               physical geometry + logical action model
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


## Universal layout and action engine

R4 makes three concepts independent:

```text
electrical matrix / device addressing
                !=
physical key geometry
                !=
logical key action
```

A `Layout` owns physical geometry and device-facing identifiers. A `KeyAction` describes behavior. Drivers translate actions into vendor protocols in later remapping runs.

### PhysicalKey

The generic model supports:

- stable key ID
- display label
- x/y position
- width/height
- rotation
- optional verified electrical matrix position
- optional source/grid position
- HID/source usage
- device key index
- LED index
- Fn-programmable flag
- logical default action

The coordinate system is arbitrary per layout. The desktop renderer scales an SVG from the layout's declared coordinate space, so the UI does not assume full-size key spacing or a fixed row count.

This is sufficient for full-size, 1800, TKL, 75%, 65%, 60%, 40%, Alice, split, ortholinear and custom layouts without changing the renderer.

### F108 layout evidence

The first layout file is `drivers/aula-f108pro/layouts/f108pro-ansi.json`.

It contains 104 physical key records derived from the analyzed OEM layout definition:

- physical rectangles
- HID/source codes
- `key_index`
- `light_index`
- `fnlayer_disable`
- OEM `row_col`

Important evidence boundary:

> OEM `row_col` is stored as `grid_position`. MechKeyMesh does not claim that it is the electrical switch matrix.

Therefore every F108 `matrix` value remains null until the real electrical matrix is independently verified.

The original proprietary OEM XML and artwork are not redistributed. The repository stores only the interoperability facts needed for our independent layout implementation.

### KeyAction

The first universal action schema includes:

```text
Default
Disabled
KeyboardKey
ModifiedKey
Media
Mouse
Windows
OpenApplication
OpenFile
OpenUrl
SendText
Macro
Layer
VendorSpecific
```

Future variants such as TapHold, Transparent, OneShot, Chord and TapDance can be added without changing physical layout geometry.

### Layout registry

Drivers declare supported `layout_ids` in their manifest. Layouts are validated and registered independently from device sessions.

The desktop requests a registered layout from the Tauri backend and renders it from data. No F108 key positions are hard-coded in HTML or JavaScript.


## Hardware lighting effect metadata

R5 moves built-in lighting mode behavior out of UI assumptions and into driver-provided metadata.

A driver can expose `LightingEffectDescriptor` records describing:

- numeric mode ID
- display name
- color behavior: none, color-or-rainbow, or built-in-rainbow-only
- brightness support
- speed support
- optional direction axis

The desktop builds the lighting controls from this metadata. It does not maintain an AULA-specific switch statement for which controls appear.

For the AULA F108 Pro, the 20 effect definitions are based on the OEM-derived `config_func` mode table documented by the existing F108 reverse-engineering work. Direction axis is stored explicitly per mode because the recovered bit labels and the mode table must not be conflated.

The F108 packet builder also normalizes unused fields to zero. Hidden/stale UI values therefore cannot leak into parameters that an effect does not use.

Examples:

```text
Off
  no adjustable parameters

Static
  color or rainbow
  brightness

Colourful / Spectrum
  built-in rainbow
  brightness
  speed

Scrolling
  color or rainbow
  brightness
  speed
  left/right direction

Rolling / Rotating / Flowing / Tilt
  color or rainbow
  brightness
  speed
  up/down direction
```

Physical effect behavior remains a separate evidence level. Software metadata and successful HID ACKs do not make an effect physically verified.


## Normal and Fn remap layers

R7 treats the two F108 remap tables as genuinely independent desired-state capabilities:

```text
Normal layer
  capability: keys
  command:    04 11

Fn layer
  capability: fn-layer
  command:    04 27
```

Each layer owns a complete 576-byte table. Both use the verified remap trailer bytes `AA 55` at offsets 574-575. The driver selects the layer-specific init command and keeps the remaining table encoding shared.

The database scopes the layers separately:

```text
device model + profile + Keys    + normal
device model + profile + FnLayer + fn
```

Normal and Fn mappings therefore survive independently and cannot overwrite each other in persistence.

### Fn native-control protection

The F108 OEM layout XML contains 104 keys and does not mark any of them with a non-zero `fnlayer_disable` value. That is evidence about the OEM software restriction table, not proof that the firmware has no important native Fn combinations.

MechKeyMesh therefore adds a distinct driver safety policy for firmware functions that should remain available by default. Protected Fn assignments include native reset, connection-mode, OS-mode, lighting, Windows-lock and the physical Fn selector shortcuts. The multifunction knob is not part of the remap table at all, so its LCD navigation and Fn+knob behavior are left firmware-owned.

A protected Fn key may always be restored to `Default`, but the driver rejects non-default assignments for it.

### Fn activation mode evidence

OEM binary analysis maps the `fn_switch` setting as:

```text
0 = Toggle
1 = Momentary
```

MechKeyMesh exposes that enum as evidence metadata, but does not write it in R7. The associated `04 17 / 00 01` function-settings transaction is not physically verified, and the current device value is not reliably readable.

### Reset semantics

MechKeyMesh intentionally separates safe remap restoration from broader OEM reset terminology:

- Reset selected: editor operation only; selected assignments become Default.
- Reset current layer: persist and send an empty 576-byte table for only Normal or Fn.
- Reset key assignments: atomically clear both desired tables, then apply Normal and Fn through the serialized hardware manager.
- Reset Keyboard: not exposed until its full OEM/hardware scope is verified.
- Factory Reset: not exposed until its full OEM/hardware scope is verified.

A hardware application failure after persistence does not discard the desired reset state. The database remains authoritative and can be reconciled later.
