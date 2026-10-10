# MechKeyMesh Manifesto
## A complete blueprint for a stable, safe, full-featured replacement for the AULA F108 Pro software

This document consolidates and corrects everything established so far from four sources:

1. **Your exact AULA F108 Pro Beta 1.0.0.2 software package**
2. **Parsiya's `f108-pro` reverse engineering**
3. **Punkster81's `AULA-F108-Driver`**
4. **Features and architectural improvements we have designed ourselves**

The objective is **not another experimental RGB utility**.

The objective is:

> **A complete replacement for the AULA F108 Pro software that preserves everything the keyboard already does, reuses proven open-source work, adds the best community features, fixes the OEM application's reliability problems, and makes dangerous operations impossible by design.**

---

# 1. Product identity

Official name:

# **MechKeyMesh**

The application should feel like software specifically designed for this keyboard, not a generic HID configurator.

Its priorities are, in this order:

**Reliability → hardware safety → complete OEM replacement → usability → advanced functionality.**

A beautiful interface that occasionally crashes or corrupts the LCD is a failure.

---

# 2. Fundamental development rule

We do **not** reinvent functioning work.

### Parsiya gives us

- Windows HID protocol
- USB device enumeration
- known packet sequences
- built-in RGB
- per-key RGB
- remapping
- normal/Fn layers
- LCD clock
- LCD transfer
- RGB565 format
- key maps
- protocol documentation
- hardware integration tests
- OEM binary reverse engineering
- LCD corruption lessons
- macro protocol documentation

Repository:

https://github.com/parsiya/f108-pro

### Punkster81 gives us

- sophisticated graphical RGB editing
- live RGB streaming
- layer system
- animation layers
- reactive effects
- static flash-to-keyboard mode
- presets
- keyboard visualization
- soundboard
- auto-update concepts
- Windows background driver architecture

Repository:

https://github.com/Punkster81/AULA-F108-Driver

### AULA gives us

The **actual specification of the product**:

- exact physical key layout
- key indices
- LED indices
- device capability flags
- normal/Fn configuration
- macros
- profiles
- application association
- RGB modes
- per-key lighting
- real-time lighting
- music lighting
- TFT editor
- GIF conversion
- clock sync
- sleep configuration
- response/debounce configuration
- Windows shortcut locking
- wired/2.4G distinctions
- firmware/update infrastructure
- onboard LCD behavior
- native knob/menu relationship

### We add

- robust device lifecycle
- transaction safety
- recovery
- better UI
- safer LCD conversion
- video import
- application profiles
- atomic state persistence
- clear hardware/software mode distinction
- diagnostics
- better update strategy
- optional future Smart Knob
- automated hardware reconciliation

---

# 3. Legal/code-source policy

Both community projects are MIT licensed.

We may reuse and modify their code as long as the relevant copyright and MIT license notices are retained.

For the OEM application:

**Do not redistribute AULA proprietary executable code, artwork, menu graphics, fonts or branded assets.**

We can independently implement compatible behavior and protocols learned through analysis.

MechKeyMesh should have its own:

- name
- icon
- artwork
- interface
- bundled assets

---

# 4. Correct conceptual model of the F108 Pro

The product consists of three distinct systems.

```text
WINDOWS APPLICATION
      │
      │ HID
      ▼
KEYBOARD CONFIGURATION ENGINE
      │
      ▼
KEYBOARD FIRMWARE
```

These must never again be conflated.

## Windows application

Handles:

- profiles
- remapping editor
- macros
- RGB editor
- music analysis
- live lighting generation
- LCD media conversion
- configuration storage
- USB communication

## Keyboard configuration engine

Accepts commands for:

- mappings
- macros
- lighting
- LCD upload
- clock
- sleep
- debounce
- various settings

## Keyboard firmware

Independently controls:

- normal keyboard operation
- onboard LCD menus
- connection status
- OS mode
- multifunction knob
- Fn shortcuts
- light bar
- side strips
- Bluetooth operation
- native configuration UI

This distinction is one of the most important lessons from the OEM investigation.

---

# 5. Supported hardware

Initial scope:

## AULA F108 Pro

Wired:

```text
VID 0C45
PID 800A
```

2.4 GHz dongle:

```text
VID 05AC
PID 024F
```

Bluetooth:

Normal keyboard operation works, but the OEM software itself does not support keyboard configuration over Bluetooth.

MechKeyMesh should **not pretend otherwise**.

---

# 6. Connection capability matrix

| Feature | USB | 2.4 GHz | Bluetooth |
|---|---:|---:|---:|
| Normal keyboard use | Yes | Yes | Yes |
| Remapping | Yes | Yes | No configuration |
| Macros | Yes | Yes | No |
| Built-in RGB | Yes | Yes | No |
| Stored per-key RGB | Yes | Yes | No |
| Live/custom RGB | Yes | Limited/No OEM custom mode | No |
| Music lighting | Yes | Yes | No |
| LCD upload | Yes | No | No |
| Clock synchronization | Yes | No | No |
| System settings | Yes | Partial, verify exact protocol | No |

The application should disable unavailable controls automatically rather than letting operations fail.

---

# 7. Application structure

The final interface should be roughly:

```text
MechKeyMesh
│
├── Home
├── Keys
├── Macros
├── Lighting
│   ├── Hardware Effects
│   ├── Per-Key RGB
│   ├── Live Effects
│   └── Music
├── LCD
├── Profiles
├── Device Settings
├── Optional Modules
│   └── Soundboard
├── Diagnostics
└── Application Settings
```

Do not bury important device controls behind obscure menus.

---

# 8. Home

The Home screen should answer immediately:

```text
AULA F108 Pro
Connected via USB

Firmware: ...
Battery: ...
Profile: Gaming

Keyboard configuration: Ready
LCD: Ready
```

Possible states:

- Disconnected
- Detecting
- Connected
- Initializing
- Ready
- Applying configuration
- LCD transfer
- Recovery
- Error

Quick actions:

- Reconnect
- Reapply profile
- Sync clock
- Open LCD
- Open lighting
- Open key configuration

---

# 9. Hardware communication architecture

This is non-negotiable.

There must be **one hardware service**.

Not:

```text
RGB talks to USB
LCD talks to USB
Macro editor talks to USB
Settings talks to USB
```

Instead:

```text
UI
 ↓
Command Manager
 ↓
Hardware Queue
 ↓
F108 HID backend
 ↓
Keyboard
```

Only the hardware backend may talk directly to HID.

---

# 10. Device state machine

```text
Disconnected
     ↓
Detected
     ↓
Opening
     ↓
Initializing
     ↓
Ready
```

During commands:

```text
Ready
 ↓
Busy
 ↓
Ready
```

Failure:

```text
Busy
 ↓
Timeout / unplug / error
 ↓
Recovery
 ↓
Close handles
 ↓
Redetect
 ↓
Reconnect
 ↓
Reconcile configuration
 ↓
Ready
```

This is how we eliminate the OEM behavior where disconnect/reconnect or Windows restart leaves the program confused.

---

# 11. HID interfaces

The wired F108 exposes separate interfaces.

## Configuration

Vendor HID interface:

```text
Usage Page FF13
64-byte Feature Reports
```

Used for:

- RGB
- mappings
- macros
- settings
- LCD control commands
- clock

## LCD data

Vendor interface:

```text
Usage Page FF68
4096-byte output report
64-byte acknowledgment
```

Used only for TFT pixel transfer.

## Standard input

Other interfaces provide:

- keyboard
- multimedia
- mouse
- knob-related firmware input

Never mix these transport layers.

---

# 12. Core transaction commands

Known general transaction commands:

```text
04 18  Begin
04 02  Apply
04 F0  Finalize
```

Readback acknowledgments are often required.

But:

# ACK does not mean the function actually worked.

This must be treated as a protocol rule.

Examples already discovered:

- invalid remap trailer was ACKed
- unsupported sidelight commands were ACKed
- destructive oversized LCD writes were ACKed

So internally distinguish:

```text
TRANSPORT_OK
DEVICE_ACK
FUNCTION_VERIFIED
```

---

# 13. Local configuration is the source of truth

The F108 protocol does not provide reliable full-state readback.

Therefore:

```text
MechKeyMesh database
      ↓
desired configuration
      ↓
keyboard
```

not:

```text
keyboard
      ↓
discover full configuration
```

The application must always know what it expects the keyboard to contain.

---

# 14. Storage architecture

Prefer SQLite.

Suggested tables/entities:

```text
devices
profiles
profile_app_rules

key_mappings
fn_mappings

macros
macro_events

hardware_lighting
per_key_lighting

live_light_presets
live_light_layers
live_light_frames

music_presets

lcd_projects
lcd_frames

device_settings

application_settings

operation_log
```

All writes should be transactional.

Important state changes should be stored **before** or safely around hardware application.

---

# 15. Configuration reconciliation

One of the strongest ideas hidden inside the OEM application is its "reapply everything" task.

We should formalize this as:

# **Reconcile Device**

After:

- reconnect
- wake
- application restart
- configuration error
- user request

the program can execute:

```text
Load active profile
 ↓
Apply device settings
 ↓
Upload macro bank
 ↓
Apply normal layer
 ↓
Apply Fn layer
 ↓
Apply stored RGB
 ↓
Apply hardware lighting
 ↓
Sync necessary runtime state
 ↓
Ready
```

Live modes then resume separately.

---

# 16. Key configuration

Two independent hardware layers:

## Normal Layer

Known command:

```text
04 11
```

## Fn Layer

Known command:

```text
04 27
```

Each layer uses a **576-byte mapping table**.

---

# 17. Key mapping actions

MechKeyMesh must reproduce the complete OEM assignment system.

### Keyboard

- normal key
- modifier
- modifier combination
- multi-key shortcut
- disabled key
- restore default

### Media

- Play/Pause
- Stop
- Previous
- Next
- Volume Up
- Volume Down
- Mute

### Mouse

- Left Click
- Middle Click
- Right Click
- Double Click
- Scroll Up
- Scroll Down
- Mouse Button 4
- Mouse Button 5

### Windows/system

- Show Desktop
- My Computer
- Lock
- Close Window
- Switch Window
- Log Off
- Sleep
- Shutdown
- Restart
- Calculator

### Browser

- Home
- Refresh
- Stop
- Back
- Forward
- Favorites
- Search

### Application actions

- Open Program
- Open File
- Open Website
- Type Text

### Advanced

- Macro
- profile switch
- custom combination

---

# 18. Key editor UX

Use the OEM physical key geometry rather than rebuilding it manually.

The original XML already provides:

- physical positions
- HID usages
- `key_index`
- `light_index`
- Fn restrictions

UI interaction should allow:

- click key
- multi-select
- drag selection
- Ctrl selection
- select row
- select WASD
- select numpad
- reset selected
- copy/paste assignment
- search actions
- undo/redo

---

# 19. Fn layer

Fn must be shown as a separate layer.

Tabs:

```text
Normal
Fn
```

The OEM supports Fn behavior configuration:

- momentary
- toggle

Some Fn combinations are firmware-reserved and should not be presented as normal programmable keys if the keyboard cannot override them.

---

# 20. System settings packet

Known mapped payload:

```text
00 01
```

Fields include:

- Disable Alt+Tab
- Disable Alt+F4
- Disable Windows key
- Fn switch mode
- Sleep time
- response/debounce level

These should live under:

# Device Settings

not be scattered around the interface.

---

# 21. Response/debounce levels

Five levels.

Approximate OEM values:

| Level | Wired | 2.4G | Bluetooth |
|---:|---:|---:|---:|
| 1 | 2 to 3 ms | 5 to 6 | 12 to 13 |
| 2 | 5 to 6 | 7 to 9 | 15 to 16 |
| 3 | 8 to 9 | 10 to 12 | 18 to 19 |
| 4 | 13 to 14 | 15 to 17 | 23 to 24 |
| 5 | 17 to 18 | 19 to 21 | 27 to 28 |

UI:

```text
1   Fastest
2   Fast
3   Balanced
4   Stable
5   Maximum debounce
```

with warning that very low debounce may cause chatter.

---

# 22. Sleep

Known OEM options:

- Never
- 1 minute
- 5 minutes
- 30 minutes

Exact raw enum values should be confirmed through controlled packet capture before we declare implementation complete.

---

# 23. Reset semantics

We should improve the OEM terminology.

## Reset Key Assignments

Resets mapping tables.

Do not call this "reset keyboard" if it does not actually reset everything.

## Factory Reset

Broader configuration/default restoration.

Before implementing, verify exact physical effects so our label is accurate.

---

# 24. Macro system

This should be a first-class module.

## Recording

Capture:

- key down
- key up
- mouse down
- mouse up
- timing

Filters:

- Keyboard
- Mouse
- Keyboard + Mouse

---

# 25. Macro editor

Timeline concept:

```text
0 ms     W down
87 ms    W up
116 ms   Mouse Left down
155 ms   Mouse Left up
350 ms   Ctrl+C
```

Functions:

- record
- stop
- insert
- delete
- reorder
- duplicate
- edit delay
- multi-select
- copy/paste
- save
- rename
- clone
- export
- import
- test

---

# 26. Macro timing

Modes:

- recorded delays
- no delays
- fixed interval

Additional MechKeyMesh improvements:

- playback speed multiplier
- scale selected delays
- normalize timings
- total duration display

---

# 27. Macro playback behavior

Support OEM behaviors where confirmed:

- once
- fixed repeat count
- repeat until pressed again
- stop on release
- click/trigger behavior

The UI mentions up to 255 loops, but hardware limits require validation.

Do not hard-code that assumption until tested.

---

# 28. Macro hardware protocol

Known USB structure:

```text
04 19
 ↓
04 15
 ↓
macro bank
 ↓
04 02
```

The macro bank contains up to 100 indexed macro slots.

Macro execution in a key mapping uses action type:

```text
06
```

The actual macro index and playback parameters follow it.

---

# 29. Macro memory safety

Before writing:

- validate total serialized size
- validate macro count
- validate event count
- validate delay range
- verify every key code
- refuse invalid buffers

Never depend on firmware bounds checking.

---

# 30. Built-in hardware RGB

Twenty F108 effects:

1. Off
2. Static
3. SingleOn
4. SingleOff
5. Glittering
6. Falling
7. Colourful
8. Breath
9. Spectrum
10. Outward
11. Scrolling
12. Rolling
13. Rotating
14. Explode
15. Launch
16. Ripples
17. Flowing
18. Pulsating
19. Tilt
20. Shuttle

Parameters differ by effect:

- color
- rainbow
- brightness
- speed
- direction

---

# 31. Hardware lighting UI

Show only parameters valid for that effect.

For example:

```text
Static
Brightness
Color
```

not:

```text
Static
Brightness
Speed
Direction
```

if those properties do not apply.

The OEM already encoded this with capability bits.

We should preserve the idea.

---

# 32. Stored per-key RGB

Known command family:

```text
04 23
```

RGB representation:

```text
light_index
R
G
B
```

Features:

- individual keys
- multi-selection
- regions
- palettes
- gradients
- copy/paste
- presets
- import/export

---

# 33. Hardware lighting vs live lighting

This distinction must be explicit.

## Hardware Lighting

Stored/executed by keyboard.

Advantages:

- continues after MechKeyMesh closes
- low/no CPU
- works before Windows login

## Live Lighting

Generated continuously by MechKeyMesh.

Advantages:

- advanced animations
- reactive typing
- music
- application-aware behavior
- complex layers

The user should always know which mode they are using.

---

# 34. Live RGB engine

Reuse concepts/code where appropriate from Punkster81.

Layer types:

- Static
- Animation
- Reactive
- Audio

Each layer:

- enabled/disabled
- opacity
- blend mode
- order
- preset

---

# 35. Reactive effects

At minimum preserve Punkster's:

- Key Highlight
- Ripple
- Meteor
- Lightning

Then add:

- typing trail
- wave
- explosion
- heat map
- rain
- matrix
- sparkle
- breathing regions
- gradient pulse
- rainbow ripple

---

# 36. OEM real-time effects

The original program also implements live effects such as:

- Static On
- Starlight
- Fluttering
- Dynamic Breathing
- Colorful Fountain
- Rainbow Wave
- Following Currents
- Peak Revolving
- Turn Off Lighting

The goal is eventually to cover these as well.

---

# 37. Live lighting transport

Known family:

```text
04 20
```

Do not implement live lighting by constantly rewriting persistent RGB memory.

Use the actual live-streaming protocol.

---

# 38. Music lighting

Reproduce OEM functionality:

- Rhythm
- Amplitude
- background mode
- background brightness
- spectrum styles
- rainbow
- reverse rainbow
- gradient
- white/red/orange/yellow/green/cyan/blue/purple
- Ambilight-style behavior

Additional features:

- audio device selector
- smoothing
- falloff
- bass sensitivity
- mids
- treble
- stereo
- FFT visualization

Audio source behavior must be validated before claiming microphone support.

---

# 39. Light bar and side strips

Do **not** provide fake software controls.

F108 configuration explicitly disables the generic AULA sidelight module:

```text
sidelight="0"
```

Generic commands exist, but they do not control these lights on F108 Pro.

Instead provide:

# Hardware Shortcuts

and explain the relevant Fn combinations.

If a real F108 control protocol is later discovered, then software controls can be added.

---

# 40. LCD module

LCD specification:

```text
240 × 135 pixels
RGB565
141 safe frames maximum
1 user animation slot
256-byte animation header
```

This subsystem requires stronger safety guarantees than anything else in the application.

---

# 41. LCD project editor

MechKeyMesh should reproduce the OEM mini graphics editor.

### Project

- New
- Open
- Save
- Duplicate
- Delete

### Media

- PNG
- JPG/JPEG
- BMP if useful
- GIF
- WebP
- animated WebP
- video import

### Transform

- Fit
- Fill
- crop
- resize
- rotate
- flip
- center
- background

### Drawing

- text
- font
- size
- color
- brush
- brush thickness
- eraser
- undo
- redo
- reset frame

---

# 42. LCD animation timeline

Every frame:

```text
thumbnail
duration
index
```

Functions:

- duplicate
- delete
- reorder
- multi-select
- duration
- apply duration to selected
- apply duration to all
- playback preview

Always show:

```text
Frames: 74 / 141
Duration: 6.2 s
Flash usage: ...
Safety: SAFE
```

---

# 43. Safe media converter

User should not need to prepare keyboard-compatible files manually.

Input:

```text
GIF
video
images
```

Pipeline:

```text
decode
 ↓
crop/resize
 ↓
240 × 135
 ↓
frame optimisation
 ↓
≤ 141 frames
 ↓
safe timing
 ↓
RGB565
 ↓
validate
 ↓
preview
 ↓
upload
```

---

# 44. Video import

Example:

Original:

```text
1920 × 1080
60 FPS
10 seconds
600 frames
```

MechKeyMesh can automatically create:

```text
240 × 135
~14 FPS
10 seconds
141 frames
```

or shorten the clip for smoother playback.

Offer:

### Smooth

Higher FPS, shorter clip if needed.

### Balanced

Moderate FPS and duration.

### Full duration

Keep all time, reduce frame rate.

---

# 45. Intelligent frame reduction

Do not simply keep every Nth frame.

Prefer motion-aware reduction:

- compare neighboring frames
- discard visually redundant frames
- preserve high-motion transitions
- adjust duration accordingly

This improves long GIF/video conversion substantially.

---

# 46. Absolute LCD safety rule

# **Never upload more than 141 frames.**

No:

```text
Ignore warning
Force upload
Advanced override
```

The F108 firmware does not protect its own flash boundaries.

Oversized transfers can overwrite onboard menu graphics.

This has already happened on real hardware.

---

# 47. LCD payload validation

Before USB communication begins:

```text
width == 240
height == 135
frames >= 1
frames <= 141
pixel format == RGB565
header valid
timings valid
calculated payload == actual payload
page count valid
device == supported F108
LCD interface available
no competing LCD transaction
```

Only then:

**Send to Keyboard**

---

# 48. LCD hardware transfer

Correct pipeline:

```text
04 18
 ↓
04 72
 ↓
4096-byte page
 ↓
ACK
 ↓
page
 ↓
ACK
 ...
 ↓
04 02
```

LCD data must use its dedicated interrupt/output interface.

Never use the normal 64-byte configuration transport for image pages.

---

# 49. LCD transfer cancellation

If interrupted by:

- unplug
- timeout
- user cancellation
- Windows sleep

the application should:

1. stop sending immediately
2. release handles
3. mark LCD transfer incomplete
4. reconnect
5. warn the user
6. not automatically resume unless the protocol is proven safely resumable

---

# 50. LCD clock

Button:

# Sync Clock

Optional setting:

```text
Sync LCD clock when MechKeyMesh starts
```

Known command:

```text
04 28
```

Date payload includes year/month/day/hour/minute/second/weekday.

---

# 51. Preserve the native LCD UI

The LCD is **not merely a GIF screen**.

The firmware includes its own:

- GIF page
- clock/status page
- configuration menus

Known menu categories include:

- Connect
- System
- Effect
- Colour
- Brightness
- Speed
- Volume
- Language

MechKeyMesh must never overwrite or interfere with those menu regions.

---

# 52. Native knob behavior

Normal behavior on your physical keyboard:

### Rotate

Navigate onboard LCD pages and menus.

### Fn + Rotate

Volume control.

### Fn + Press

Media stop on your firmware/configuration.

This native behavior should be preserved.

---

# 53. Knob policy

MechKeyMesh **does not intercept the knob by default**.

The keyboard's own LCD navigation is useful and should remain independent of the application.

---

# 54. Future Smart Knob

Optional experimental feature only after native behavior has been captured and understood.

Possible application-aware actions:

- timeline scrub
- browser tabs
- brush size
- app volume
- zoom
- scrolling

But it must never silently break the keyboard's own menu navigation.

Therefore:

```text
Experimental
Smart Knob
[Off]
```

default.

---

# 55. Profiles

A profile should contain:

```text
Profile
├── normal mappings
├── Fn mappings
├── macros
├── device settings
├── hardware RGB
├── per-key RGB
├── live RGB preset
├── music preset
└── optional LCD assignment
```

Examples:

- Default
- Gaming
- Work
- Photoshop
- Premiere
- Coding
- Night

---

# 56. Application-linked profiles

The OEM database has application association fields.

MechKeyMesh should implement this cleanly:

```text
Photoshop.exe → Photoshop profile
premiere.exe  → Premiere profile
game.exe      → Gaming profile
```

When an application activates:

```text
detect foreground process
 ↓
select matching profile
 ↓
apply only changed hardware state
```

Avoid re-uploading macros/LCD unnecessarily.

---

# 57. Profile switching optimization

Use configuration diffing.

Bad:

```text
profile changes
→ rewrite everything
```

Better:

```text
compare old vs new
 ↓
send only changed subsystems
```

Example:

If only RGB changed:

- do not rewrite remap tables
- do not resend macros
- do not touch LCD

---

# 58. Soundboard

Keep Punkster's soundboard as an optional module.

Features:

- audio files
- key combinations
- volume
- rename cards
- global triggers
- output selection
- virtual audio cable support

This is useful but **not core keyboard configuration**, so it should not clutter the primary experience.

---

# 59. System tray

Required:

- minimize to tray
- start minimized
- open MechKeyMesh
- active profile
- reconnect
- pause live effects
- exit

The tray icon must always render correctly.

---

# 60. Startup

Option:

```text
Start MechKeyMesh with Windows
```

Prefer standard user-level startup configuration.

No unnecessary Windows service unless we eventually have a feature that actually requires one.

---

# 61. Auto-update

MechKeyMesh can implement its own updater.

Required safeguards:

- signed/checksummed release package
- release notes
- user confirmation
- rollback where feasible
- never update firmware during normal application update

Application and keyboard firmware are completely different concerns.

---

# 62. Firmware

Initial releases should display:

```text
Firmware version: ...
Firmware updating: unavailable
```

Do not include flashing until the process is independently understood and tested.

Firmware mistakes can brick hardware.

---

# 63. Battery/status

AULA has generic battery infrastructure and the LCD displays battery information.

MechKeyMesh should eventually expose:

- battery %
- charging
- wired/wireless state

But first reverse-engineer and validate the F108-specific report path.

Until then:

```text
Battery: unavailable
```

is better than fabricated data.

---

# 64. Diagnostics

Dedicated Diagnostics page:

```text
Device
VID/PID
connection
interfaces
firmware
open handles

Configuration
active profile
pending commands

LCD
interface
last transfer
frame/page count

Communication
last command
last ACK
timeouts
retry count

Application
version
database
logs
```

Button:

# Export Diagnostic Report

No sensitive unrelated user information should be included.

---

# 65. Logging

Levels:

- Error
- Warning
- Info
- Debug
- USB trace, optionally

Example:

```text
15:43:02 Device detected 0C45:800A
15:43:02 Interface FF13 opened
15:43:02 Interface FF68 opened
15:43:03 Device Ready

15:45:14 Apply RGB profile "Gaming"
15:45:14 04 18 ACK
15:45:14 04 13 ACK
15:45:14 Payload sent
15:45:14 04 02 ACK
```

Logs should make failures diagnosable.

---

# 66. Reliability rules

Every operation must have:

- timeout
- cancellation
- error handling
- bounded retries

Never infinite retry loops.

Never block the GUI thread.

Never allow multiple hardware writers simultaneously.

---

# 67. USB unplug/replug

Expected experience:

```text
Keyboard disconnected
```

then:

```text
Keyboard detected
Restoring configuration...
Ready
```

No application restart.

---

# 68. Windows sleep/wake

On sleep:

- suspend live effects
- stop new transfers
- release/cancel pending I/O safely

On wake:

- redetect
- reopen
- reconcile
- resume live effects

---

# 69. Competing applications

Detect likely exclusive access failures.

Message:

> MechKeyMesh cannot access the keyboard configuration interface. Another keyboard utility may currently be using it.

Provide:

**Retry**

Do not just crash.

---

# 70. Data safety

Configuration save strategy:

```text
write new data transaction
 ↓
validate
 ↓
commit
```

Never partially overwrite the configuration database.

Keep lightweight automatic backups for major version migrations.

---

# 71. Undo/redo

Editors that should support undo:

- mappings
- RGB
- macros
- LCD frames

Hardware should not necessarily update after every tiny editor operation.

Use:

```text
Preview
Apply
```

where appropriate.

---

# 72. Development stack

The strongest starting architecture is:

## Backend

**Go**

Why:

- Parsiya already implemented the critical protocol in Go
- native Windows HID is already solved there
- good concurrency model
- easy static distribution
- no Python runtime required
- straightforward hardware test binaries

## Desktop shell

**Wails**, provided packaging and WebView behavior pass our prototype tests.

Why:

- Go backend remains reusable
- modern HTML/CSS/JS UI
- Punkster's JavaScript concepts can be adapted
- rich editors are easier than with basic native widgets

If Wails proves unreliable in testing, the backend should remain independent enough that another UI layer can replace it.

---

# 73. Required internal separation

```text
UI
 │
Application Services
 │
Domain Model
 │
Device Service
 │
Protocol Encoders
 │
Windows HID Backend
```

LCD media processing should be another independent subsystem:

```text
Media Importer
 ↓
Frame Processor
 ↓
Safety Validator
 ↓
F108 LCD Asset
 ↓
Device Service
```

This avoids repeating the experimental tools' coupling.

---

# 74. Community code integration strategy

Do not fork both projects into one giant codebase.

Instead extract concepts deliberately.

### From Parsiya

Prefer direct reuse/adaptation of MIT code for:

- packet structures
- HID backend
- key tables
- RGB commands
- LCD commands
- remap encoder
- tests

### From Punkster

Reuse/adapt:

- RGB effect algorithms
- layer concepts
- animation structures
- reactive effect logic
- soundboard concepts

But do not automatically inherit its entire process/shared-memory architecture.

---

# 75. OEM extraction policy

From OEM files, preserve **data knowledge**, not copyrighted presentation.

Extract/recreate:

- key geometry
- HID mappings
- feature flags
- numeric limits
- supported modes
- commands
- behavior

Do not ship:

- OEM icon
- product artwork
- menu graphic assets
- executable
- original UI artwork

---

# 76. Testing hierarchy

## Level 1: Unit tests

Test:

- serializers
- packet generators
- remap tables
- macro encoding
- RGB encoding
- RGB565 conversion
- LCD header
- frame limits

## Level 2: Golden packet tests

Compare our output against captured OEM packets.

## Level 3: Hardware integration tests

Connected F108:

- lighting
- remap
- clock
- per-key RGB

## Level 4: destructive-risk tests

Only after strong validation:

- macros
- LCD transfers
- system settings

## Level 5: recovery tests

- unplug during operation
- sleep during operation
- dongle removed
- app crashes
- competing software

---

# 77. Hardware test discipline

For every newly reverse-engineered function:

1. capture OEM command
2. document packet
3. reproduce packet
4. test smallest harmless value
5. verify physical effect
6. reverse it
7. repeat
8. add automated regression test

Do not infer working behavior merely because an ACK appears.

---

# 78. Dynamic reverse-engineering backlog

Before declaring 1.0 complete, investigate:

1. F108-specific battery reporting
2. your knob's actual HID reports
3. exact sleep enum values
4. exact Fn momentary/toggle values
5. 2.4G settings packet
6. 2.4G mapping behavior
7. full `04 F5` color readback
8. macro physical limits
9. automatic application-profile behavior
10. exact Factory Reset effects
11. firmware/version querying
12. relationship between software settings and onboard LCD Config menu

---

# 79. Features intentionally excluded from early versions

Do not include initially:

- firmware flashing
- unsupported software sidelight controls
- Bluetooth configuration
- Smart Knob override
- LCD flash recovery tools
- undocumented destructive commands

These can be research branches without contaminating the stable application.

---

# 80. Development phases

## Phase 0: Research baseline

Freeze/document:

- OEM Beta 1.0.0.2
- keyboard firmware revision
- USB descriptors
- exact physical behavior
- known packet captures

---

## Phase 1: Hardware core

Implement:

- device enumeration
- USB open/close
- command queue
- ACK handling
- timeouts
- reconnect
- logging

Exit criterion:

> MechKeyMesh can remain open during repeated keyboard unplug/replug cycles without crashing.

---

## Phase 2: Built-in lighting

Implement:

- 20 effects
- brightness
- speed
- direction
- color

Exit:

> Equal or better reliability than OEM software.

---

## Phase 3: Key mappings

Implement:

- keyboard UI
- normal layer
- Fn layer
- all known action types
- reset
- export/import

---

## Phase 4: Device settings

Implement:

- Win lock
- Alt+Tab lock
- Alt+F4 lock
- Fn behavior
- debounce
- sleep

Only after raw enum validation.

---

## Phase 5: Per-key RGB

Implement:

- stored per-key configuration
- visual editor
- palettes
- presets

---

## Phase 6: Macros

Implement:

- recording
- editor
- serialization
- onboard upload
- mapping to keys

---

## Phase 7: LCD

Implement:

- clock
- still images
- GIF
- editor
- safe uploader

This phase does not ship until the 141-frame boundary has automated tests.

---

## Phase 8: Video and advanced media

Add:

- video import
- intelligent downsampling
- motion-aware frame reduction
- better cropping
- timeline tools

---

## Phase 9: Live RGB

Implement/adapt:

- live streaming
- layers
- animation
- reactive effects

---

## Phase 10: Music mode

Implement:

- device capture
- FFT
- OEM-like modes
- advanced visualization

---

## Phase 11: Profiles

Implement:

- complete profiles
- app association
- automatic switching
- diff-based application

---

## Phase 12: 2.4 GHz parity

Validate and implement all legitimately supported wireless configuration paths.

---

## Phase 13: Product polish

- installer
- startup
- tray
- updater
- diagnostics
- crash reports
- translations
- documentation

---

# 81. Release philosophy

## Alpha

For us.

Risky operations visible only after testing.

## Public Alpha

Core:

- detection
- RGB
- per-key RGB
- remapping
- profiles
- safe LCD basics

must already be reliable.

## Beta

All common OEM functionality.

No known hardware-danger bug.

## 1.0

MechKeyMesh should reasonably replace the OEM application for daily use.

---

# 82. Definition of "OEM replacement"

Version 1.0 should allow the user to uninstall/ignore AULA software and still have:

- key assignment
- Fn layer
- macros
- built-in RGB
- per-key RGB
- custom/live RGB
- music effects
- LCD media
- clock sync
- debounce
- sleep
- Windows key locks
- profiles
- USB operation
- supported 2.4G configuration
- tray/startup
- stable reconnect

That is the bar.

---

# 83. Definition of "better"

MechKeyMesh is not better simply because it contains more buttons.

It becomes better when:

### AULA crashes

MechKeyMesh recovers.

### Keyboard disconnects

MechKeyMesh reconnects.

### User imports an unsafe GIF

MechKeyMesh automatically converts it safely.

### User changes application

MechKeyMesh switches profiles correctly.

### Hardware does not support something

MechKeyMesh says so rather than exposing a broken control.

### A command times out

MechKeyMesh reports and recovers rather than freezing.

### User wants advanced RGB

MechKeyMesh offers layers and reactive effects.

That is genuine improvement.

---

# 84. The most important safety invariant

This should literally exist as a developer comment and test:

> **No user action through MechKeyMesh may generate an LCD write outside the documented F108 Pro user-media region.**

And another:

> **No undocumented or unverified hardware command may enter the stable release merely because the keyboard ACKs it.**

---

# 85. Product principles

The project should follow seven permanent rules:

1. **Reuse proven work before creating new implementations.**
2. **Physical hardware behavior outranks assumptions from static analysis.**
3. **Device-specific configuration outranks generic AULA code.**
4. **An ACK proves transport, not functionality.**
5. **Dangerous hardware limits are enforced, never optional.**
6. **Native keyboard firmware features must continue working without MechKeyMesh.**
7. **Stability is a feature, not cleanup work for the end.**

---

# 86. Ultimate target

The finished system should look conceptually like:

```text
                  MECHKEYMESH
                       │
        ┌──────────────┼──────────────┐
        │              │              │
     Profiles        Editors      Live Engines
        │              │              │
        └──────────────┼──────────────┘
                       │
                 Desired State
                       │
                 Device Manager
                       │
                Command Queue
                       │
              Protocol Validator
                       │
        ┌──────────────┴──────────────┐
        │                             │
 Configuration HID               LCD HID
        │                             │
        └──────────────┬──────────────┘
                       │
                AULA F108 PRO
                       │
             Native firmware remains
                 fully functional
```

And the user's experience should simply be:

> **Connect keyboard → MechKeyMesh recognizes it → configure anything → it works → close the app if desired → onboard features remain functional → reopen later and continue.**

That is the project.

This should now be treated as the **master development manifesto and single source of truth** for the MechKeyMesh project. Future discoveries should amend this specification rather than creating parallel, contradictory plans.
