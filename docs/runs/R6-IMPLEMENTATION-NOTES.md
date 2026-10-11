# R6 implementation notes

## Implemented software scope

- normal layer only
- complete 576-byte F108 remap table
- transactional desired-state persistence
- keyboard keys
- standalone modifiers
- modifier shortcuts
- verified media actions
- verified mouse actions
- verified Windows/system actions
- disable and restore-default actions
- multi-select
- reset selected
- copy/paste
- undo/redo

## Deliberately disabled

The following action families exist in the generic MechKeyMesh action model but are not enabled for the F108 R6 driver:

- Open application
- Open file
- Open URL
- Send text
- Macro
- Fn-layer action
- Mouse double-click
- Mouse buttons 4 and 5
- Log off
- Sleep
- Shutdown
- Restart

The OEM application/file/URL/text mappings use an indexed host-side handler. The four-byte keyboard mapping alone is insufficient, so those controls remain disabled until that host event channel is independently implemented and verified.

## Protocol boundary

The normal mapping transaction sends the 576-byte table as nine 64-byte reports without an extra table readback, matching the current working open-source implementation. Begin, init, apply and finalize commands retain their verified readbacks.

The table trailer remains exactly `AA 55` at offsets 574 and 575.

## Evidence boundary

Automated tests can prove packet construction, bounds, serialization, persistence and application build integrity. They cannot prove that pressing a remapped physical key produces the requested Windows input.

R6 was closed on 2026-10-11 by project decision with physical verification deferred. The evidence boundary is unchanged: the milestone is complete, but physical remapping reliability is not yet claimed.
