# Current validation status

Current test version: **0.20.0-alpha.4**. Development continues from `main` after [branch consolidation](CONSOLIDATION-2026-10-14.md). Repository cleanup does not close physical acceptance gates.

## Automated evidence

[CI run 154](https://github.com/ManethonSega/MechKeyMesh/actions/runs/38426430321) passed at `d5858f41f86c588d3d06ff4bd687c912e755361e`, including Linux core tests/Clippy, Windows workspace tests/Clippy, the Chrome macro-library regression, branding/format/syntax checks and a Windows release build. That head includes the latest user-observed knob record. Consolidation changes documentation and preserves history; its exact head is checked again by CI before merge.

## Acceptance gates

| Run / feature | Software status | Physical evidence and remaining checks | Complete |
| --- | --- | --- | --- |
| R1-R4 platform and layout | Previously accepted implementation and R4 hardening | Hardware-specific follow-up belongs to the rows below | Yes |
| R5 onboard lighting | All 20 modes mapped | Later user logs show successful writes; all effects and persistence still need physical checks | No |
| R6-R7 Normal/Fn and assignment reset | Closed by project decision with physical verification deferred | Startup transfers now complete in user logs; actual assignments and reset behavior remain unverified | Yes, project closure only |
| R8 stored RGB | Editor/persistence implemented; alpha.4 reply/flash corrections included | Last alpha.3 test failed; alpha.4 key-to-LED mapping, colours and persistence pending | No |
| R9 local macro engine/library | Persistence, editing and simulation implemented; alpha.4 browser regression passes | Hardware-independent; user's alpha.4 macro-button smoke test pending | No, user acceptance pending |
| R10 hardware macros | Preflight, stable slots, matching data readback and async 15-second manual upload | Startup transfers now complete; execution, bindings and power-cycle persistence pending | No |
| R11 device settings | Validated desired-state settings implemented | Change/verify/restore checks pending | No |
| R12 2.4 GHz | Discovery, reduced capability set and battery reporting implemented | USB/2.4 GHz/USB switching and representative changes pending | No |
| R13 live RGB | Layered engine and bounded queue; alpha.4 frame/recovery corrections included | Alpha.3 repeated reply failures recorded; alpha.4 effects and prolonged streaming pending | No |
| R14 music lighting | WASAPI/FFT and five modes implemented | Windows endpoint selection, audio sync and prolonged hardware run pending | No |
| R15 LCD media engine | Hardware-independent import/edit/compile and format limits tested | No physical hardware required; software implementation included | Yes, software scope |
| R16 LCD upload and clock | Dedicated interface, preflight, cancellation and clock-echo handling implemented | User confirmed clock sync physically works; LCD uploads complete in logs; repeated upload matrix pending | No |
| R17 profiles | Cloning, native application picker, matching and output diffing implemented | Physical switching and redundant-traffic capture pending | No |
| R18 native hardware | Observation reporting and evidence ledger implemented | User confirmed knob/menu/volume actions; raw reports, Fn+rotation, firmware/revision and menu correlations pending | No |
| R19 Windows integration | Tray, startup preferences, single instance and power lifecycle implemented | Interactive Windows behavior and 72-hour background test pending | No |

See each `R*.md` run record for its detailed exit criterion, and [alpha.4 feedback](F108-2026-10-13-ALPHA4.md) for the latest failures/fixes. Historical run records describe their original scope; this page and the current architecture describe the consolidated version. Unknown observations remain unknown. Successful builds, ACKs and completed transport logs do not establish physical functionality.
