# Repository consolidation, 2026-10-14

## Scope and retained implementation

The user requested GitHub cleanup, consolidation of outdated branches, and a project update. The audited default branch was `578f32be2195249d14e71599c070631b80ce85a1` (R7 / 0.8.0-alpha.1). The current cumulative implementation is `d5858f41f86c588d3d06ff4bd687c912e755361e` (0.20.0-alpha.4), including the user's 2026-10-13 knob observations.

The consolidation uses that latest implementation unchanged, updates the README/status/protocol notes, and retains the earlier branch heads in Git ancestry. The R8-R19 and fix branches are already ancestors of the current implementation. Nine older branches were squash-merged in PRs #1-#9, so their original commit IDs were not ancestors of `main`. Their heads are additional parents of the history-preservation commit; their obsolete file contents are not reapplied.

Eight older branch trees exactly match their corresponding merged PR snapshot. The `r4/layout-action-engine` branch differs from its PR #4 snapshot only in the Windows GUI subsystem attribute in `main.rs`, already supplied by the separately merged #5 hotfix and present in the current implementation. Thus no additional feature changes are required to retain those branches' work.

The old Go/Wails prototype is also an ancestor of the current implementation and remains accessible at its recorded SHA below. Branch pointers can be removed after the consolidated commit is validated and merged. Every recorded branch head must be verified as an ancestor of final `main` before deletion. Existing PR descriptions remain historical evidence, with superseded work closed as consolidation rather than physical milestone acceptance.

## Audited branch heads

| Superseded branch | Retained commit |
| --- | --- |
| `archive/go-wails-prototype` | `841481d298a3c13719f0c009bb033869cc598c7a` |
| `fix/f108-feature-replies` | `73748073a92ab45942e6dde82498272906e8941f` |
| `fix/f108-macro-data-readback` | `668489803e1977f6ab18c95d329bc7241ecb124d` |
| `fix/f108-rgb-macro-library` | `d5858f41f86c588d3d06ff4bd687c912e755361e` |
| `fix/original-logo` | `253c1c48680684fc1e0e0d6689bc783ddce87e3d` |
| `fix/profile-application-picker` | `4aa39a94c9393d140e53f2c68e1a45f8f73b1f9e` |
| `hardening/r4-layout-action-engine` | `f608598e96e597be97d43016550baf078e5fe7c4` |
| `hotfix/windows-gui-subsystem` | `9b8a84895e62addd612b5f4daed8ff899f0ddb3d` |
| `migration/rust-tauri` | `aa0663817abb1db27c42ca86e29400b7305970ec` |
| `r10/f108-hardware-macros` | `043f844faf96ab2bc40ff989ac5325539720ae14` |
| `r11/device-settings` | `16e207e86812135c00ed313cb232cb329ee33126` |
| `r12/f108-24g-support` | `f8ad74b7fa6fe0489467ee741d203a0865806991` |
| `r13/live-rgb-engine` | `ad0549d7f7de69e025ae49a535c2731203b4f7d9` |
| `r14/music-lighting` | `a4d1006aafc51fed291033d5eaa9a616d01203c2` |
| `r15/lcd-media-engine` | `234260ecd94af3566580aa13c07ed83303066ea5` |
| `r16/lcd-hardware-upload` | `90ea02fc0eb4a94cc67080178bab42b77298354c` |
| `r17/profiles-auto-switching` | `3e1b49bc9a2bc8f7861bcd36ce23822c7c72b11f` |
| `r18/native-hardware-integration` | `3d5cfff21dea0572ec2177d8f1aecab732413fcd` |
| `r19/windows-integration` | `82f2a49ecf34ce027cadc6957b6b00d60cb5707e` |
| `r2/device-manager-2` | `ef4db171daf69aa325ce22d296ae0b4f9ab8aa90` |
| `r3/persistence` | `1e59ff0cb27fdbdf757602e473730b78834677f9` |
| `r4/layout-action-engine` | `ef1c6e299395f20ddb473da3e0c3ef20169ff6c5` |
| `r5/f108-onboard-rgb` | `76189b9caf838b4bfb7105cf7aa59dc7e498b145` |
| `r6/normal-layer-remapping` | `a575b669f03d837d44372ad76c8e44d64ec65ccd` |
| `r7/fn-layer-reset-semantics` | `76c28b99dd7dcaec15a9a99b6be585de23592740` |
| `r8/stored-per-key-rgb` | `39ab9a6613aca7c1de4d766374b4894d466e0fd7` |
| `r9/universal-macro-engine` | `c622153330476433042a67b9fee94f8838ce0d3b` |

## Validation and open acceptance

The source implementation passed [CI run 154](https://github.com/ManethonSega/MechKeyMesh/actions/runs/38426430321). The consolidation PR runs the full CI suite again before merge. Linux/Windows tests, Clippy, format/syntax, approved transparent branding, Chrome macro-library behavior and Windows executable production remain the required automated checks.

Consolidation does not make this an independently hardware-verified stable release. The [current status ledger](STATUS.md) keeps the user-observed clock/knob behavior separate from transport completion and leaves outstanding RGB, macro, transport/profile switching and background-soak checks pending. Continue subsequent development from `main` with one task branch at a time.
