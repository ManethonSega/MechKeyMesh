# R7 hardware verification matrix

**Device:** AULA F108 Pro  
**Capability:** Normal/Fn remapping and safe assignment reset  
**Status:** PHYSICAL VERIFICATION PENDING

ACKs and successful CI are not physical passes.

| Test | Expected physical result | Result | Notes |
|---|---|---|---|
| Normal mapping on a test key | Key produces assigned action without Fn | Pending | |
| Fn mapping on an unprotected test key | Default action without Fn; assigned Fn action while Fn is active | Pending | |
| Normal/Fn independence | Changing Fn assignment does not change Normal assignment and vice versa | Pending | |
| Fn layer persistence after app close | Fn assignment remains in keyboard behavior after MechKeyMesh exits | Pending | |
| Fn layer persistence after app restart | Desired Fn assignment reloads from SQLite and remains independently editable | Pending | |
| Reset selected on Normal | Only selected Normal assignment returns to default after save | Pending | |
| Reset selected on Fn | Only selected Fn assignment returns to default after save | Pending | |
| Reset current Normal layer | All Normal remaps return to firmware default; Fn remaps remain unchanged | Pending | |
| Reset current Fn layer | All Fn remaps return to firmware default; Normal remaps remain unchanged | Pending | |
| Reset key assignments | Both Normal and Fn remaps return to firmware default; unrelated lighting/LCD state is not intentionally touched | Pending | |
| Fn+Esc | Native factory-reset shortcut remains available because key is protected | Pending | |
| Fn+grave / Fn+1 / Fn+2 / Fn+3 / Fn+4 | Native connection controls remain available | Pending | |
| Fn+Q / Fn+W / Fn+E / Fn+R | Native OS-mode controls remain available | Pending | |
| Fn+Tab / Fn+Backslash / Fn+arrows | Native main-light controls remain available | Pending | |
| Fn+Left Shift/Ctrl/Alt/Z | Native light-bar controls remain available | Pending | |
| Fn+Right Shift/Ctrl/Alt/Slash | Native sidelight controls remain available | Pending | |
| Fn+Win | Windows-lock behavior remains available | Pending | |
| Knob rotation/press | Native LCD/knob navigation remains unchanged | Pending | |
| Fn+knob | Native knob-mode behavior remains unchanged | Pending | |
| Disconnect/reconnect | Desired Normal and Fn states remain stored without corruption | Pending | |

## Fn activation mode

OEM evidence maps:

```text
0 = Toggle
1 = Momentary
```

R7 intentionally does not write the Fn activation setting yet. A future hardware test must separately verify the `04 17 / 00 01` settings packet before MechKeyMesh enables this control.

## Reset terminology

This matrix verifies only MechKeyMesh mapping reset operations.

It does not claim that the OEM "Reset Keyboard" or "Factory Reset" semantics are fully reproduced. Those names remain unavailable in the UI until their complete physical effects are verified.
