# MechKeyMesh architecture

## Design goals

MechKeyMesh is not hard-wired to one keyboard. A device-specific driver owns protocol details, while the application core owns lifecycle, serialization, state, recovery and user-facing workflows.

```text
Frontend (Wails WebView)
        |
        v
Application API
        |
        v
Device Manager
  |             |
  |             +--> Diagnostics / log buffer
  v
Serialized Command Queue
        |
        v
Protocol Driver Interface
        |
        +--> AULA F108 Pro
        +--> future device drivers
        |
        v
OS Transport (Windows HID)
```

## Current Phase 1 boundaries

The first implementation deliberately supports only low-risk capabilities:

- F108 Pro wired USB detection
- robust open/close lifecycle
- reconnect attempts after disconnect/failure
- 20 onboard lighting effects
- diagnostics

Not yet enabled:

- LCD writes
- remapping
- macros
- device settings
- firmware flashing
- 2.4 GHz configuration
- live RGB streaming

Those are not UI omissions. They are intentionally gated until their protocol and safety tests are in place.

## Device manager

`internal/device.Manager` is the only application service allowed to hold an open device driver. UI calls are converted into queued operations. This prevents RGB, LCD, macros and settings from talking to HID concurrently as the feature set grows.

Lifecycle states:

```text
Disconnected -> Detecting -> Opening -> Ready
                                 ^        |
                                 |        v
                              Recovery <- Busy
```

The manager periodically attempts reconnection when no device is open. A failed hardware operation invalidates the active device handle and returns the manager to recovery/discovery rather than leaving stale handles in the application.

## Driver interface

`internal/protocol.Device` is intentionally small. Device-specific capabilities are added through optional interfaces, for example `LightingDevice`. This allows a future keyboard that supports remapping but not RGB to participate without pretending it has unsupported capabilities.

## Protocol encoders

Packet construction is separated from transport. Payload builders can be unit tested without a physical keyboard. Hardware tests are a separate layer.

## OEM compatibility rule

Generic code found in an OEM executable is not automatically treated as a feature of a specific keyboard. Device configuration, physical testing and packet captures take precedence over generic shared code.

## ACK rule

A HID acknowledgment means only that transport succeeded. It does not prove the hardware acted on the request. Features move from experimental to stable only after a functional hardware test.
