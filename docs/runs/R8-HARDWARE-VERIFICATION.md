# R8 hardware verification matrix

**Device:** AULA F108 Pro  
**Capability:** Stored per-key RGB  
**Status:** PHYSICAL VERIFICATION PENDING

A successful ACK, CI run, or F5 readback is not by itself proof that the intended physical LED illuminated.

| Test | Expected physical result | Result | Notes |
|---|---|---|---|
| Esc red | Only Esc is red | Pending | LED index 1 |
| F12 green | Only F12 is green | Pending | LED index 13 |
| Backspace blue | Only Backspace is blue | Pending | LED index 103 |
| Print Screen orange | Only Print Screen is orange | Pending | LED index 112 |
| Numpad + purple | Only Numpad + is purple | Pending | LED index 123 |
| Arrow cluster distinct colors | Left/Down/Up/Right correspond to the intended physical arrows | Pending | LED indices 99/100/101/102 |
| WASD preset | W/A/S/D are the selected physical keys and receive the chosen color | Pending | |
| Row selection | Selected source-grid row lights exactly that physical row | Pending | |
| Numpad preset | Only the data-defined numpad section is selected/colored | Pending | |
| Region drag | Keys physically inside the dragged region are selected | Pending | |
| Multi-select | Ctrl/Cmd-selected keys retain independent selection | Pending | |
| Palette | Chosen palette swatch is applied to selected keys | Pending | |
| Gradient | Selected keys show the intended start-to-end progression | Pending | |
| Copy one color / paste many | All target keys receive the copied color | Pending | |
| Copy multi / paste matching multi | Ordered target selection receives the copied color sequence | Pending | |
| Clear selected | Selected LEDs turn off; other stored colors remain | Pending | |
| Brightness 0-5 | Overall stored-RGB brightness follows the chosen level | Pending | |
| Empty design | All main per-key LEDs turn off | Pending | |
| App close | Stored per-key colors remain active after MechKeyMesh exits | Pending | |
| App restart | Desired colors and brightness reload from SQLite correctly | Pending | |
| Disconnect/reconnect | Desired state remains intact without database corruption | Pending | |
| 04 F5 readback | Read editor colors correspond to the current physical stored colors | Pending | |
| F5 does not auto-save | Reading hardware state changes the editor only until Save is pressed | Pending | |

## Persistence evidence

For at least one multi-color design:

1. Save and apply the design.
2. Photograph or record representative physical keys.
3. Close MechKeyMesh completely.
4. Verify the same colors remain on the keyboard.
5. Reopen MechKeyMesh.
6. Verify the desired colors and brightness reload.
7. Use `Read keyboard colors`.
8. Confirm the read values match the visible physical state.
9. Change the editor after readback without saving.
10. Restart the app and confirm the database state, not the unsaved readback edit, reloads.

## Completion rule

R8 closes only when representative LEDs across low, middle, high and sparse F108 `light_index` values are physically confirmed and persistence behaves correctly.
