# Current validation status

Current test version: **0.20.0-alpha.6**. This update revises the desktop UI: grouped navigation, task-focused overview, theme-aware controls, progressive disclosure and explicit draft/save/send feedback. The original transparent logo, native file pickers and hardware protocols are preserved. See [UI revision](UI-2026-10-15.md). R18 remains closed as a bounded investigation with explicitly unavailable capabilities. Development continues from `main` after [branch consolidation](CONSOLIDATION-2026-10-14.md). Repository cleanup does not close physical acceptance gates.

## Automated evidence

[CI run 170](https://github.com/ManethonSega/MechKeyMesh/actions/runs/38724089072) passed for alpha.6 at `b4504e03f933d7d638c9c4ced6663d723c181aad`. This includes 144 Linux core tests, 149 Windows workspace tests, Clippy on both platforms, formatting, frontend syntax, transparent-original branding checks, the existing macro-library regression and the full desktop UI regression.

The UI regression checked all 12 pages in both themes at 1320×840, eight editor/overview pages in both themes at the 1040×680 minimum window, visible control labels, larger text, draft preservation, save/failure/retry states, in-flight save locking, delete cancellation/confirmation, native-picker commands, keyboard layer tabs, reduced motion, startup recovery and USB/2.4 GHz capability changes. Tauri/hardware commands were mocked. It captured 43 screenshots; representative pages, both themes and compact layouts were visually reviewed. The [UI screenshot artifact](https://github.com/ManethonSega/MechKeyMesh/actions/runs/38724089072/artifacts/11966364626) records the final render batch.

Windows download: [MechKeyMesh-Windows-test artifact](https://github.com/ManethonSega/MechKeyMesh/actions/runs/38724089072/artifacts/11967401926), containing `MechKeyMesh.exe` and `MechKeyMesh.exe.sha256`. The downloaded ZIP matches the GitHub artifact digest, `7fe7f51f5f44b99e3338db3e402077f68d0d749d8cdf8859fd2c5538b34cefd9`. The executable matches its sidecar: `6e8dd7a2fc739b321792ec1e0f35366f193f9db30e73fcb77dfb3601b42a196e`.

The alpha.6 UI revision is **complete as a software change**. Interactive Windows acceptance and physical F108 testing remain separate. Charging detection and other unavailable native readbacks are not claimed as functional. The hardware acceptance gates below are unchanged. Earlier alpha.5 build evidence is retained in the repository history.

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
| R18 native hardware | Closed investigation; Home charging status and clear unavailable/readback reporting included | USB volume reports captured; wireless behavior and battery display confirmed by user; firmware/revision, charging decoder and automatic menu readback remain unavailable/deferred | Yes, bounded investigation; limitations documented |
| R19 Windows integration | Tray, startup preferences, single instance and power lifecycle implemented | Interactive Windows behavior and 72-hour background test pending | No |

See each `R*.md` run record for its detailed exit criterion, and [alpha.4 feedback](F108-2026-10-13-ALPHA4.md) for the latest failures/fixes. Historical run records describe their original scope; this page and the current architecture describe the consolidated version. Unknown observations remain unknown. Successful builds, ACKs and completed transport logs do not establish physical functionality.

