# Phase 1: hardware core

## Goal

Build the boring foundation first: MechKeyMesh must survive device absence, unplug/replug and command failures without blocking the UI or corrupting internal state.

## Implemented in the first alpha

- [x] device lifecycle model
- [x] central device manager
- [x] serialized operation queue
- [x] periodic reconnect loop
- [x] F108 wired device identity
- [x] Windows HID feature-report transport
- [x] built-in lighting payload validation and sender
- [x] application diagnostics/log buffer
- [x] Wails shell and initial UI
- [x] backend unit tests for core packet validation

## Hardware validation checklist

On the target Windows PC:

- [ ] application starts with keyboard disconnected
- [ ] connecting the F108 moves state to Ready without restart
- [ ] unplugging returns state to Disconnected/Recovery without crash
- [ ] reconnecting returns to Ready
- [ ] all 20 built-in lighting modes apply
- [ ] brightness bounds 0..5 are enforced
- [ ] speed bounds 0..5 are enforced
- [ ] invalid mode values never reach USB
- [ ] OEM AULA application being open produces a readable access error instead of a crash
- [ ] repeated reconnect cycles remain stable

## Phase 1 exit criterion

> MechKeyMesh can remain open during repeated keyboard unplug/replug cycles and normal lighting changes without crashing, freezing or requiring an application restart.
