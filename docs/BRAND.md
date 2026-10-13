# MechKeyMesh brand system

## Product name

**MechKeyMesh**

Descriptor: **Universal Keyboard Studio**

Optional short brand line: **Connect · Configure · Control**

## Official color palette

| Token | Hex | Primary use |
|---|---|---|
| Mesh Cyan | `#21E6E8` | primary interactive accent, selection, active states |
| Neon Aqua | `#18D7F1` | secondary accent, mesh/network identity |
| Electric Blue | `#1C8DFF` | gradients, focus, links, live/device activity |
| Deep Navy | `#081327` | primary dark application background |
| Carbon Slate | `#1A2436` | panels, surfaces, neutral dark UI |
| Silver White | `#EDF2F7` | primary light text and neutral surface |
| Accent Orange | `#FF922E` | warnings, exceptional attention, small brand accent |

The cyan/aqua/blue family is the primary identity. Orange is intentionally rare so it keeps semantic weight.

## Logo

The master logo is the futuristic mechanical-keycap/shield mark with:

- silver `M` monogram
- dark navy mechanical body
- cyan-to-blue network/mesh motif
- four network nodes
- small orange side accent

The approved master is `apps/desktop/frontend/dist/assets/logo.png`, an unchanged copy of the supplied **MechKeyMesh Futuristic Keycap Logo(1)(2).png**. Use this exact artwork everywhere; do not redraw it or substitute an approximation. This updated original has transparent surroundings. Preserve its alpha channel and the opaque silver `M`; never restore a white rectangle around it.

## Application icon family

1. **Square app icon**: the original logo fitted proportionally into a square, preserving transparency and adding only transparent padding.
2. **UI logo**: the unchanged original PNG, retaining its aspect ratio and transparent surroundings.
3. **Tray icon**: the same original artwork resized to 32 px; no separate replacement symbol.
4. **Installer icon**: use the normal app mark. A separate download arrow is optional in installer UI, but the executable/install target should retain the normal app identity.

Windows `.ico` output includes 16, 20, 24, 32, 40, 48, 64, 128 and 256 px variants. Tauri embeds it into the executable and uses the configured icon family for windows. The tray PNG is embedded independently. Splash, sidebar and favicon reference the original master directly.

To regenerate the icon family, install Pillow and run `python tools/brand_assets.py`. Run `python tools/brand_assets.py --check` to verify every generated PNG/ICO against the transparent original, its approved checksum, its alpha channel, and the frontend/tray/config references. CI performs this check. The supplied transparent master is used directly, with no cropping, recoloring or redesign.

## Startup branding

Preferred startup treatment:

- Deep Navy to near-black background
- master logo centered slightly above the midpoint
- `MechKeyMesh` wordmark
- `UNIVERSAL KEYBOARD STUDIO` descriptor
- subtle mesh/circuit lines
- small status message such as `Initializing devices…`

Keep the startup screen short. It should communicate initialization, not hide a slow launch.


## Dark and light application themes

Both themes use the same official palette rather than introducing unrelated theme colors.

### Dark mode

- application background: Deep Navy `#081327`
- panels and key surfaces: Carbon Slate `#1A2436` plus darker derived surfaces
- primary text: Silver White `#EDF2F7`
- selection and active states: Mesh Cyan `#21E6E8`
- secondary activity/focus: Neon Aqua `#18D7F1` and Electric Blue `#1C8DFF`
- warnings: Accent Orange `#FF922E`

### Light mode

- application background and neutral areas: Silver White `#EDF2F7`
- primary text and strong structure: Deep Navy `#081327`
- dark neutral accents/borders: Carbon Slate `#1A2436`
- selection and interactive accents remain Mesh Cyan, Neon Aqua and Electric Blue
- warnings remain Accent Orange

The keyboard renderer, navigation, buttons, status surfaces and selection states must inherit semantic theme variables. Device/layout data must never contain UI colors.
