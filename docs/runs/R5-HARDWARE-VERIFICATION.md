# R5 hardware verification matrix

**Device:** AULA F108 Pro  
**Capability:** Built-in/onboard hardware RGB  
**Status:** DEFERRED - R5 CLOSED WITHOUT PHYSICAL VERIFICATION

This document remains the evidence record for physical F108 onboard-RGB validation. R5 was closed by explicit user direction before these tests were performed. A successful HID transaction, ACK, CI run, or software test is not a physical pass.

| Mode | Effect | Expected accepted parameters | Visible result | Persists after app closes | Result | Notes |
|---:|---|---|---|---|---|---|
| 0 | Off | None | Pending | Pending | Pending | |
| 1 | Static | Color/Rainbow, Brightness | Pending | Pending | Pending | |
| 2 | SingleOn | Color/Rainbow, Brightness, Speed | Pending | Pending | Pending | |
| 3 | SingleOff | Color/Rainbow, Brightness, Speed | Pending | Pending | Pending | |
| 4 | Glittering | Color/Rainbow, Brightness, Speed | Pending | Pending | Pending | |
| 5 | Falling | Color/Rainbow, Brightness, Speed | Pending | Pending | Pending | |
| 6 | Colourful | Built-in rainbow, Brightness, Speed | Pending | Pending | Pending | |
| 7 | Breath | Color/Rainbow, Brightness, Speed | Pending | Pending | Pending | |
| 8 | Spectrum | Built-in rainbow, Brightness, Speed | Pending | Pending | Pending | |
| 9 | Outward | Color/Rainbow, Brightness, Speed | Pending | Pending | Pending | |
| 10 | Scrolling | Color/Rainbow, Brightness, Speed, Left/Right | Pending | Pending | Pending | |
| 11 | Rolling | Color/Rainbow, Brightness, Speed, Up/Down | Pending | Pending | Pending | |
| 12 | Rotating | Color/Rainbow, Brightness, Speed, Up/Down | Pending | Pending | Pending | |
| 13 | Explode | Color/Rainbow, Brightness, Speed | Pending | Pending | Pending | |
| 14 | Launch | Color/Rainbow, Brightness, Speed | Pending | Pending | Pending | |
| 15 | Ripples | Color/Rainbow, Brightness, Speed | Pending | Pending | Pending | |
| 16 | Flowing | Color/Rainbow, Brightness, Speed, Up/Down | Pending | Pending | Pending | |
| 17 | Pulsating | Color/Rainbow, Brightness, Speed | Pending | Pending | Pending | |
| 18 | Tilt | Color/Rainbow, Brightness, Speed, Up/Down | Pending | Pending | Pending | |
| 19 | Shuttle | Color/Rainbow, Brightness, Speed | Pending | Pending | Pending | |

## Test procedure

For each row:

1. Select the effect in MechKeyMesh.
2. Confirm that only the expected controls are visible.
3. Apply a clearly distinguishable test configuration.
4. Observe the physical keyboard behavior.
5. Change every exposed parameter through at least two values and confirm that it changes the expected property.
6. For directional modes, test both direction values.
7. For color-or-rainbow modes, test one single color and rainbow/colorful mode.
8. Close MechKeyMesh completely.
9. Confirm whether the effect continues on the keyboard.
10. Reopen MechKeyMesh and move to the next mode.

Do not mark a mode Passed merely because the app reports success.

## Deferred verification rule

R5 is closed, but this matrix is still intentionally incomplete.

A future claim that the F108 onboard RGB capability is **physically verified** requires every row to have:

- a visible physical result
- persistence result
- Pass or Fail
- sufficient notes to explain any deviation

Until then, the capability is implemented and software-tested, not physically verified.
