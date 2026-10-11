# R4 hardening audit: universal layout and action engine

## Status

**IMPLEMENTED, CI PENDING**

This audit revisits the completed R4 architecture after R6 was merged. It does not reopen the original milestone. It strengthens the generic layout/action foundation before additional keyboard shapes and vendors depend on it.

## Diagnosis

The R4 separation was fundamentally correct:

```text
electrical/device addressing
!=
physical key position
!=
logical action
```

The audit found five areas that needed stronger guarantees.

### 1. Layout schema versions were not enforced

A layout with an unknown `schemaVersion` could pass validation and be interpreted by an older application.

### 2. Geometry validation was too permissive

The original validator rejected negative positions and non-positive sizes, but it did not reject every NaN/infinite value or keys extending outside the declared coordinate space.

### 3. Rotation was limited to each key's center

A single rotation angle is enough for ordinary rectangular keyboards, but Alice, split and unusual custom layouts often rotate several keys around a shared pivot. The renderer and model therefore needed an optional explicit rotation origin.

### 4. Driver layout ownership was incomplete

The driver registry compared declared and provided layout IDs, but it did not guarantee that:

- every provided layout was valid
- a layout belonged to the driver's device model
- two drivers could not claim the same global layout ID
- layouts were cached after validation instead of being regenerated repeatedly

### 5. The desktop selected the first driver's layout

That was acceptable for the first F108-only build but not for a real multi-vendor platform. Layout selection now follows the active device's driver, with the first registered layout used only as the disconnected fallback.

## Improvements implemented

### Layout model

- explicit supported schema version
- optional `section` identifier
- optional `rotationOrigin`
- matrix and source-grid `unit` identifiers for split/multi-controller keyboards
- helper for stable key lookup
- helper for layouts by device model

### Validation

The validator now rejects:

- unsupported schema versions
- empty layout names
- empty key IDs or labels
- NaN/infinite coordinate spaces and key geometry
- invalid rotation origins
- keys outside the declared coordinate space after rotation
- duplicate matrix positions within the same matrix unit
- duplicate source-grid positions within the same grid unit
- duplicate device or LED indices
- invalid default-action payloads

### KeyAction validation

The generic action model now rejects:

- reserved keyboard usage zero
- modified-key actions without modifiers
- duplicate modifiers
- empty application/file/URL/text/macro/layer/vendor identifiers

Standalone modifiers remain representable through a modified-key action with a non-empty modifier set and key usage zero.

### Driver registry

Driver registration is now atomic and validates/caches layouts before the driver becomes visible. It rejects device-model mismatches and global layout-ID collisions.

### Renderer

The SVG renderer now:

- uses an explicit rotation origin when supplied
- falls back to the key center for existing layouts
- scales key labels to available key dimensions
- refreshes the layout when the active driver changes
- exposes selection through `aria-pressed`
- includes schema version, section, matrix unit, grid unit and rotation-pivot information in the UI

### F108 evidence checks

The embedded F108 layout test now confirms that all 104 keys retain:

- source label
- HID/source usage
- device key index
- LED index
- source-grid position
- Fn programmability evidence

It also checks the recorded OEM XML SHA-256 fingerprint and verifies that the unproven electrical matrix remains unset.

## Compatibility

The new fields are optional/defaulted. The existing `aula.f108pro.ansi` schema-1 JSON remains valid without modification.

The F108 geometry, device indices, LED indices and action persistence format are unchanged.

## Acceptance

This hardening is accepted only after:

- Rust formatting passes
- all workspace tests pass
- Linux and Windows Clippy pass with warnings denied
- Windows desktop release build passes
- the executable is verified and uploaded by CI
