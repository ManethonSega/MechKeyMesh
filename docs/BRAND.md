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

The current repository SVG is a lightweight scalable implementation for the development UI. Before public release, replace it with the final approved vector master while preserving the same silhouette and brand palette.

## Application icon family

1. **Square app icon**: logo centered on a rounded Deep Navy square, generous safe area.
2. **Transparent logo**: standalone mark with no background for About, branding and documentation.
3. **Tray icon**: simplified high-contrast mark. At 16–24 px, prioritise the `M`, outer silhouette and cyan top edge. Fine network details may be reduced.
4. **Installer icon**: use the normal app mark. A separate download arrow is optional in installer UI, but the executable/install target should retain the normal app identity.

Windows `.ico` output should eventually include at minimum 16, 20, 24, 32, 40, 48, 64, 128 and 256 px variants.

## Startup branding

Preferred startup treatment:

- Deep Navy to near-black background
- master logo centered slightly above the midpoint
- `MechKeyMesh` wordmark
- `UNIVERSAL KEYBOARD STUDIO` descriptor
- subtle mesh/circuit lines
- small status message such as `Initializing devices…`

Keep the startup screen short. It should communicate initialization, not hide a slow launch.
