use super::{AulaF108ProSession, COMMAND_DELAY, validate_ack};
use mechkeymesh_driver_api::{
    DriverError, KeyAction, KeyAssignment, KeyRestriction, KeymapCapabilities, KeymapLayer,
    MediaAction, Modifier, MouseAction, OperationContext, WindowsAction,
};
use mechkeymesh_hid::FEATURE_REPORT_SIZE;
use std::collections::HashSet;

const REMAP_TABLE_SIZE: usize = 0x240;
const REMAP_TRAILER_OFFSET: usize = REMAP_TABLE_SIZE - 2;
const NORMAL_REMAP_COMMAND: u8 = 0x11;
const FN_REMAP_COMMAND: u8 = 0x27;

const ACTION_SPECIAL: u8 = 0x01;
const ACTION_KEY: u8 = 0x02;
const ACTION_CONSUMER: u8 = 0x03;
const ACTION_HOST_SPECIAL: u8 = 0x05;
const ACTION_MACRO: u8 = 0x06;
const ACTION_MOUSE: u8 = 0x07;

pub fn keymap_capabilities(layer: KeymapLayer) -> KeymapCapabilities {
    KeymapCapabilities {
        disable: true,
        keyboard_key: true,
        modifier: true,
        shortcut: true,
        media_actions: vec![
            MediaAction::PlayPause,
            MediaAction::Stop,
            MediaAction::Previous,
            MediaAction::Next,
            MediaAction::VolumeUp,
            MediaAction::VolumeDown,
            MediaAction::Mute,
        ],
        mouse_actions: vec![
            MouseAction::LeftClick,
            MouseAction::MiddleClick,
            MouseAction::RightClick,
            MouseAction::ScrollUp,
            MouseAction::ScrollDown,
        ],
        windows_actions: vec![
            WindowsAction::ShowDesktop,
            WindowsAction::Computer,
            WindowsAction::Lock,
            WindowsAction::CloseWindow,
            WindowsAction::SwitchWindow,
            WindowsAction::Calculator,
        ],
        open_application: false,
        open_file: false,
        open_url: false,
        send_text: false,
        macro_action: true,
        protected_keys: match layer {
            KeymapLayer::Normal => vec![],
            KeymapLayer::Fn => fn_protected_keys(),
        },
        unsupported_host_actions_reason: Some(
            "The OEM program/file/URL/text actions use indexed host-side handlers. The F108 packet alone is not enough, so MechKeyMesh keeps them disabled until the host event channel is independently verified."
                .into(),
        ),
    }
}

pub fn fn_protected_keys() -> Vec<KeyRestriction> {
    let mut protected = Vec::new();
    let mut add = |device_key_index: u16, reason: &str| {
        protected.push(KeyRestriction {
            device_key_index,
            reason: reason.into(),
        });
    };

    add(1, "Fn+Esc is the native firmware factory-reset shortcut.");
    for index in 19..=23 {
        add(index, "Native firmware connection-mode shortcut.");
    }
    for index in 38..=41 {
        add(index, "Native firmware OS-mode shortcut.");
    }
    for index in [
        37, 67, 73, 74, 78, 83, 84, 91, 92, 93, 95, 98, 99, 100, 101, 102,
    ] {
        add(
            index,
            "Native firmware lighting or keyboard-control shortcut.",
        );
    }
    add(96, "The physical Fn key controls access to the Fn layer.");

    protected.sort_by_key(|restriction| restriction.device_key_index);
    protected.dedup_by_key(|restriction| restriction.device_key_index);
    protected
}

fn remap_command(layer: KeymapLayer) -> u8 {
    match layer {
        KeymapLayer::Normal => NORMAL_REMAP_COMMAND,
        KeymapLayer::Fn => FN_REMAP_COMMAND,
    }
}

fn remap_init(layer: KeymapLayer) -> [u8; FEATURE_REPORT_SIZE] {
    let mut init = [0u8; FEATURE_REPORT_SIZE];
    init[0] = 0x04;
    init[1] = remap_command(layer);
    init[8] = 0x09;
    init
}

pub fn validate_keymap(
    layer: KeymapLayer,
    assignments: &[KeyAssignment],
) -> Result<(), DriverError> {
    let protected: HashSet<u16> = if layer == KeymapLayer::Fn {
        fn_protected_keys()
            .into_iter()
            .map(|restriction| restriction.device_key_index)
            .collect()
    } else {
        HashSet::new()
    };

    let mut seen = HashSet::new();
    for assignment in assignments {
        validate_device_index(assignment.device_key_index)?;
        if !seen.insert(assignment.device_key_index) {
            return Err(DriverError::InvalidConfiguration(format!(
                "duplicate F108 device key index {}",
                assignment.device_key_index
            )));
        }
        if protected.contains(&assignment.device_key_index)
            && assignment.action != KeyAction::Default
        {
            return Err(DriverError::InvalidConfiguration(format!(
                "F108 Fn key index {} is protected to preserve native firmware behavior",
                assignment.device_key_index
            )));
        }
        encode_action(&assignment.action)?;
    }
    Ok(())
}

pub fn build_remap_table(
    layer: KeymapLayer,
    assignments: &[KeyAssignment],
) -> Result<[u8; REMAP_TABLE_SIZE], DriverError> {
    validate_keymap(layer, assignments)?;
    let mut table = [0u8; REMAP_TABLE_SIZE];

    for assignment in assignments {
        let offset = usize::from(assignment.device_key_index) * 4;
        table[offset..offset + 4].copy_from_slice(&encode_action(&assignment.action)?);
    }

    // The remap table owns a little-endian uint16 0x55AA trailer.
    // This is deliberately AA 55 on the wire and must not be generalized to
    // other F108 command families.
    table[REMAP_TRAILER_OFFSET] = 0xAA;
    table[REMAP_TRAILER_OFFSET + 1] = 0x55;
    Ok(table)
}

pub fn send_keymap(
    session: &mut AulaF108ProSession,
    context: &OperationContext,
    layer: KeymapLayer,
    assignments: &[KeyAssignment],
) -> Result<(), DriverError> {
    let table = build_remap_table(layer, assignments)?;

    session.begin_transaction(context)?;
    session.send(context, remap_init(layer), true)?;
    send_multi_packet(session, context, &table, false)?;
    session.apply_transaction(context)?;

    let mut finalize = [0u8; FEATURE_REPORT_SIZE];
    finalize[0] = 0x04;
    finalize[1] = 0xF0;
    session.send(context, finalize, true)?;
    Ok(())
}

pub fn send_normal_keymap(
    session: &mut AulaF108ProSession,
    context: &OperationContext,
    assignments: &[KeyAssignment],
) -> Result<(), DriverError> {
    send_keymap(session, context, KeymapLayer::Normal, assignments)
}

pub fn send_fn_keymap(
    session: &mut AulaF108ProSession,
    context: &OperationContext,
    assignments: &[KeyAssignment],
) -> Result<(), DriverError> {
    send_keymap(session, context, KeymapLayer::Fn, assignments)
}

fn send_multi_packet(
    session: &AulaF108ProSession,
    context: &OperationContext,
    data: &[u8],
    readback: bool,
) -> Result<(), DriverError> {
    if data.is_empty() || !data.len().is_multiple_of(FEATURE_REPORT_SIZE) {
        return Err(DriverError::Internal(format!(
            "multi-packet payload length {} is not a positive multiple of {}",
            data.len(),
            FEATURE_REPORT_SIZE
        )));
    }
    for chunk in data.chunks_exact(FEATURE_REPORT_SIZE) {
        context.check()?;
        let mut report = [0u8; FEATURE_REPORT_SIZE];
        report.copy_from_slice(chunk);
        session
            .transport
            .set_feature_report(&report)
            .map_err(super::map_transport_error)?;
        context.sleep(COMMAND_DELAY)?;
    }

    if readback {
        let response = session
            .transport
            .get_feature_report()
            .map_err(super::map_transport_error)?;
        validate_ack(&response)?;
        context.sleep(COMMAND_DELAY)?;
    }
    Ok(())
}

fn validate_device_index(index: u16) -> Result<(), DriverError> {
    let offset = usize::from(index) * 4;
    if index == 0 || offset + 4 > REMAP_TRAILER_OFFSET {
        return Err(DriverError::InvalidConfiguration(format!(
            "F108 device key index {index} is outside the remap table"
        )));
    }
    Ok(())
}

fn encode_action(action: &KeyAction) -> Result<[u8; 4], DriverError> {
    match action {
        KeyAction::Default => Ok([0, 0, 0, 0]),
        KeyAction::Disabled => Ok([ACTION_HOST_SPECIAL, 0x03, 0x00, 0x00]),
        KeyAction::KeyboardKey { usage } => encode_keyboard_key(&[], *usage),
        KeyAction::ModifiedKey { modifiers, key } => encode_keyboard_key(modifiers, *key),
        KeyAction::Media { action } => {
            let usage = consumer_usage(*action);
            let [low, high] = usage.to_le_bytes();
            Ok([ACTION_CONSUMER, low, high, 0])
        }
        KeyAction::Mouse { action } => encode_mouse(*action),
        KeyAction::Windows { action } => encode_windows(*action),
        KeyAction::OpenApplication { .. }
        | KeyAction::OpenFile { .. }
        | KeyAction::OpenUrl { .. }
        | KeyAction::SendText { .. } => Err(DriverError::UnsupportedCapability(
            "F108 host-side application/file/URL/text actions are not yet verified",
        )),
        KeyAction::Macro {
            hardware_slot: Some(slot),
            ..
        } if usize::from(*slot) < crate::macro_bank::SLOT_COUNT => {
            Ok([ACTION_MACRO, *slot, 1, 0])
        }
        KeyAction::Macro {
            hardware_slot: Some(slot),
            ..
        } => Err(DriverError::InvalidConfiguration(format!(
            "F108 macro slot {slot} is outside the 100-slot bank"
        ))),
        KeyAction::Macro {
            hardware_slot: None,
            ..
        } => Err(DriverError::InvalidConfiguration(
            "macro assignment was not resolved to an F108 bank slot".into(),
        )),
        KeyAction::Layer { .. } => Err(DriverError::UnsupportedCapability(
            "layer actions are not part of F108 normal-layer remapping",
        )),
        KeyAction::VendorSpecific { .. } => Err(DriverError::UnsupportedCapability(
            "unverified vendor-specific remap action",
        )),
    }
}

fn encode_keyboard_key(modifiers: &[Modifier], key: u16) -> Result<[u8; 4], DriverError> {
    if key > u16::from(u8::MAX) {
        return Err(DriverError::InvalidConfiguration(format!(
            "keyboard HID usage 0x{key:04X} does not fit the F108 remap slot"
        )));
    }

    let mut mask = modifier_mask(modifiers);
    let key = key as u8;
    if (0xE0..=0xE7).contains(&key) {
        mask |= modifier_usage_bit(key);
        return Ok([ACTION_KEY, mask, 0, 0]);
    }
    if key == 0 && mask == 0 {
        return Err(DriverError::InvalidConfiguration(
            "keyboard remap requires a key or modifier".into(),
        ));
    }
    Ok([ACTION_KEY, mask, key, 0])
}

fn modifier_mask(modifiers: &[Modifier]) -> u8 {
    modifiers.iter().fold(0, |mask, modifier| {
        mask | match modifier {
            Modifier::LeftCtrl => 0x01,
            Modifier::LeftShift => 0x02,
            Modifier::LeftAlt => 0x04,
            Modifier::LeftMeta => 0x08,
            Modifier::RightCtrl => 0x10,
            Modifier::RightShift => 0x20,
            Modifier::RightAlt => 0x40,
            Modifier::RightMeta => 0x80,
        }
    })
}

const fn modifier_usage_bit(usage: u8) -> u8 {
    1 << (usage - 0xE0)
}

const fn consumer_usage(action: MediaAction) -> u16 {
    match action {
        MediaAction::PlayPause => 0x00CD,
        MediaAction::Stop => 0x00B7,
        MediaAction::Previous => 0x00B6,
        MediaAction::Next => 0x00B5,
        MediaAction::VolumeUp => 0x00E9,
        MediaAction::VolumeDown => 0x00EA,
        MediaAction::Mute => 0x00E2,
    }
}

fn encode_mouse(action: MouseAction) -> Result<[u8; 4], DriverError> {
    match action {
        MouseAction::LeftClick => Ok([ACTION_MOUSE, 0x01, 0x00, 0x00]),
        MouseAction::RightClick => Ok([ACTION_MOUSE, 0x02, 0x00, 0x00]),
        MouseAction::MiddleClick => Ok([ACTION_MOUSE, 0x04, 0x00, 0x00]),
        MouseAction::ScrollUp => Ok([ACTION_MOUSE, 0x00, 0x01, 0x00]),
        MouseAction::ScrollDown => Ok([ACTION_MOUSE, 0x00, 0xFF, 0x00]),
        MouseAction::DoubleClick | MouseAction::Button4 | MouseAction::Button5 => Err(
            DriverError::UnsupportedCapability("this F108 mouse remap encoding is not verified"),
        ),
    }
}

fn encode_windows(action: WindowsAction) -> Result<[u8; 4], DriverError> {
    match action {
        WindowsAction::ShowDesktop => Ok([ACTION_KEY, 0x08, 0x07, 0]),
        WindowsAction::Computer => Ok([ACTION_KEY, 0x08, 0x08, 0]),
        WindowsAction::Lock => Ok([ACTION_KEY, 0x08, 0x0F, 0]),
        WindowsAction::CloseWindow => Ok([ACTION_KEY, 0x01, 0x1A, 0]),
        WindowsAction::SwitchWindow => Ok([ACTION_KEY, 0x04, 0x2B, 0]),
        WindowsAction::Calculator => Ok([ACTION_SPECIAL, 0x03, 0x01, 0]),
        WindowsAction::LogOff
        | WindowsAction::Sleep
        | WindowsAction::Shutdown
        | WindowsAction::Restart => Err(DriverError::UnsupportedCapability(
            "this F108 Windows/system remap encoding is not verified",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assignment(index: u16, action: KeyAction) -> KeyAssignment {
        KeyAssignment {
            device_key_index: index,
            action,
        }
    }

    #[test]
    fn normal_and_fn_layers_use_distinct_verified_init_commands() {
        let normal = remap_init(KeymapLayer::Normal);
        let fn_layer = remap_init(KeymapLayer::Fn);
        assert_eq!(&normal[..2], &[0x04, 0x11]);
        assert_eq!(&fn_layer[..2], &[0x04, 0x27]);
        assert_eq!(normal[8], 0x09);
        assert_eq!(fn_layer[8], 0x09);
    }

    #[test]
    fn fn_layer_rejects_protected_native_firmware_shortcuts() {
        for index in [1u16, 19, 20, 23, 38, 41, 73, 83, 92, 96, 102] {
            assert!(
                validate_keymap(KeymapLayer::Fn, &[assignment(index, KeyAction::Disabled)])
                    .is_err(),
                "protected Fn key index {index} must not be remappable"
            );
            assert!(
                validate_keymap(KeymapLayer::Fn, &[assignment(index, KeyAction::Default)]).is_ok(),
                "default must remain a safe restore operation for {index}"
            );
        }
    }

    #[test]
    fn fn_layer_accepts_unreserved_assignments() {
        let table = build_remap_table(
            KeymapLayer::Fn,
            &[assignment(55, KeyAction::KeyboardKey { usage: 0x04 })],
        )
        .unwrap();
        assert_eq!(&table[55 * 4..55 * 4 + 4], &[0x02, 0x00, 0x04, 0x00]);
        assert_eq!(&table[574..576], &[0xAA, 0x55]);
    }

    #[test]
    fn builds_complete_table_with_little_endian_trailer() {
        let table =
            build_remap_table(KeymapLayer::Normal, &[assignment(55, KeyAction::Disabled)]).unwrap();
        assert_eq!(table.len(), REMAP_TABLE_SIZE);
        assert_eq!(&table[55 * 4..55 * 4 + 4], &[0x05, 0x03, 0, 0]);
        assert_eq!(&table[574..576], &[0xAA, 0x55]);
    }

    #[test]
    fn encodes_keyboard_modifier_and_shortcut_actions() {
        assert_eq!(
            encode_action(&KeyAction::KeyboardKey { usage: 0x04 }).unwrap(),
            [0x02, 0x00, 0x04, 0x00]
        );
        assert_eq!(
            encode_action(&KeyAction::KeyboardKey { usage: 0xE0 }).unwrap(),
            [0x02, 0x01, 0x00, 0x00]
        );
        assert_eq!(
            encode_action(&KeyAction::ModifiedKey {
                modifiers: vec![Modifier::LeftCtrl],
                key: 0x06,
            })
            .unwrap(),
            [0x02, 0x01, 0x06, 0x00]
        );
    }

    #[test]
    fn encodes_media_mouse_and_verified_windows_actions() {
        assert_eq!(
            encode_action(&KeyAction::Media {
                action: MediaAction::PlayPause,
            })
            .unwrap(),
            [0x03, 0xCD, 0x00, 0x00]
        );
        assert_eq!(
            encode_action(&KeyAction::Mouse {
                action: MouseAction::ScrollDown,
            })
            .unwrap(),
            [0x07, 0x00, 0xFF, 0x00]
        );
        assert_eq!(
            encode_action(&KeyAction::Windows {
                action: WindowsAction::ShowDesktop,
            })
            .unwrap(),
            [0x02, 0x08, 0x07, 0x00]
        );
        assert_eq!(
            encode_action(&KeyAction::Windows {
                action: WindowsAction::Calculator,
            })
            .unwrap(),
            [0x01, 0x03, 0x01, 0x00]
        );
    }

    #[test]
    fn macro_reference_requires_a_resolved_bank_slot() {
        assert_eq!(
            encode_action(&KeyAction::Macro {
                macro_id: "copy".into(),
                hardware_slot: Some(7),
            })
            .unwrap(),
            [0x06, 0x07, 0x01, 0x00]
        );
        assert!(
            encode_action(&KeyAction::Macro {
                macro_id: "copy".into(),
                hardware_slot: None,
            })
            .is_err()
        );
    }

    #[test]
    fn rejects_unverified_actions_duplicate_and_invalid_indices() {
        assert!(
            encode_action(&KeyAction::OpenUrl {
                url: "https://example.com".into(),
            })
            .is_err()
        );
        assert!(
            encode_action(&KeyAction::Mouse {
                action: MouseAction::Button4,
            })
            .is_err()
        );
        assert!(
            validate_keymap(
                KeymapLayer::Normal,
                &[
                    assignment(1, KeyAction::Disabled),
                    assignment(1, KeyAction::Disabled),
                ],
            )
            .is_err()
        );
        assert!(
            validate_keymap(KeymapLayer::Normal, &[assignment(0, KeyAction::Disabled)]).is_err()
        );
    }

    #[test]
    fn default_action_leaves_slot_zeroed() {
        let table =
            build_remap_table(KeymapLayer::Normal, &[assignment(1, KeyAction::Default)]).unwrap();
        assert_eq!(&table[4..8], &[0, 0, 0, 0]);
    }
}
