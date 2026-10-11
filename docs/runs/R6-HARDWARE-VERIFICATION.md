# R6 hardware verification matrix

**Device:** AULA F108 Pro  
**Layer:** Normal  
**Status:** DEFERRED - R6 MILESTONE CLOSED

A successful send and ACK is not a pass. Press the physical source key and verify the actual Windows input or behavior.

R6 was closed on 2026-10-11 with this physical matrix explicitly deferred. The pending rows below remain pending, not passed, and are retained for later hardware validation.

| Test | Source key | Assignment | Expected physical result | Persists after app closes | Result | Notes |
|---|---|---|---|---|---|---|
| Default/reset | F1 | Default | F1 behaves as firmware default | Pending | Pending | |
| Keyboard key | F1 | A | Pressing F1 types A | Pending | Pending | |
| Modifier | Caps Lock | Left Ctrl | Caps Lock acts as Left Ctrl | Pending | Pending | |
| Shortcut | F2 | Ctrl+C | Pressing F2 sends Ctrl+C | Pending | Pending | |
| Disable | F3 | Disable key | Pressing F3 produces no input | Pending | Pending | |
| Media | F4 | Play/Pause | Media playback toggles | Pending | Pending | |
| Mouse | F5 | Left click | Pressing F5 performs left click | Pending | Pending | |
| Mouse scroll | F6 | Scroll down | Pressing F6 scrolls down | Pending | Pending | |
| Windows | F7 | Show Desktop | Desktop is shown, Win+D behavior | Pending | Pending | |
| Windows | F8 | Switch Window | Alt+Tab behavior occurs | Pending | Pending | |
| Windows | F9 | Calculator | Calculator opens | Pending | Pending | |
| Multi-select | F10, F11, F12 | Mute | All three keys mute/unmute audio | Pending | Pending | |
| Copy/paste | F1 copied to F2 | Same action | F2 receives copied assignment | Pending | Pending | |
| Undo/redo | F3 | Disable, undo, redo | UI and saved table follow history | Pending | Pending | |
| Unmapped-key safety | A, S, D | No mapping | Existing key behavior remains unchanged | Pending | Pending | |
| App restart | Any verified mapping | Reload desired map | UI reloads mapping after restart | Pending | Pending | |
| Full reset | Reset all mapped keys | Empty desired map | All tested keys return to defaults | Pending | Pending | |

## Procedure

1. Close the AULA OEM software before starting.
2. Select the source key in MechKeyMesh.
3. Assign the requested action.
4. Save and apply the complete normal layer.
5. Confirm that MechKeyMesh reports the transaction result.
6. Press the physical key and verify the real Windows result.
7. Repeat the action at least five times to catch intermittent behavior.
8. Close MechKeyMesh completely and test the key again.
9. Record persistence and pass/fail.
10. Reset the key before moving to a conflicting test.

## Failure discipline

Record the exact difference between expected and observed behavior. Examples:

- ACK received but key remains default
- wrong key usage
- modifier sticks or releases incorrectly
- one mapping changes another key
- mapping works only while MechKeyMesh is open
- app or keyboard requires reconnect after writing

Any such result must be logged and fixed or accurately reclassified as a remapping defect. R6 milestone closure does not convert a later hardware failure into a pass.
