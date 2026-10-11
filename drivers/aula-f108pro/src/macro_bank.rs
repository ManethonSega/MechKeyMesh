use super::{AulaF108ProSession, COMMAND_DELAY, map_transport_error, validate_ack};
use mechkeymesh_driver_api::{
    DriverError, HardwareMacroEvent, HardwareMacroMouseButton, HardwareMacroProgram,
    MacroBankDescriptor, OperationContext,
};
use mechkeymesh_hid::FEATURE_REPORT_SIZE;
use std::collections::HashSet;

pub const SLOT_COUNT: usize = 100;
pub const DIRECTORY_SIZE: usize = SLOT_COUNT * 4;
pub const FORMAT_MAX_BYTES: usize = 0xE00;
pub const USB_SAFETY_SCORE_LIMIT: usize = 2700;
pub const MAX_EVENTS_IN_SINGLE_MACRO: usize = USB_SAFETY_SCORE_LIMIT / 8 - 1;
pub const MIN_DELAY_MS: u32 = 10;
pub const MAX_DELAY_MS: u32 = u16::MAX as u32;
pub const MIN_LOOP_COUNT: u16 = 1;
pub const MAX_LOOP_COUNT: u16 = 99;

const MACRO_INIT_COMMAND: u8 = 0x19;
const DATA_HEADER_COMMAND: u8 = 0x15;
const APPLY_COMMAND: u8 = 0x02;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerializedMacroBank {
    pub payload: Vec<u8>,
    pub packet_count: u8,
}

pub fn descriptor() -> MacroBankDescriptor {
    MacroBankDescriptor {
        slot_count: SLOT_COUNT,
        maximum_bank_bytes: FORMAT_MAX_BYTES,
        maximum_events_per_macro: MAX_EVENTS_IN_SINGLE_MACRO,
        minimum_delay_milliseconds: MIN_DELAY_MS,
        maximum_delay_milliseconds: MAX_DELAY_MS,
        minimum_loop_count: MIN_LOOP_COUNT,
        maximum_loop_count: MAX_LOOP_COUNT,
        hardware_write_verified: false,
        note: "The OEM USB sender and buffer format are reverse-engineered, but the source author explicitly reports that macro upload was not tried on hardware. The complete bank is validated and serialized before transmission.".into(),
    }
}

pub fn validate_bank(programs: &[HardwareMacroProgram]) -> Result<(), DriverError> {
    serialize_bank(programs).map(|_| ())
}

pub fn serialize_bank(
    programs: &[HardwareMacroProgram],
) -> Result<SerializedMacroBank, DriverError> {
    if programs.len() > SLOT_COUNT {
        return invalid(format!(
            "F108 macro bank has {} programs; maximum is {SLOT_COUNT}",
            programs.len()
        ));
    }

    let mut ids = HashSet::new();
    let mut slots = HashSet::new();
    let mut total_events = 0usize;
    for (slot, program) in programs.iter().enumerate() {
        if program.id.trim().is_empty() {
            return invalid(format!("F108 macro slot {slot} has an empty ID"));
        }
        if !ids.insert(program.id.as_str()) {
            return invalid(format!("duplicate F108 macro ID: {}", program.id));
        }
        if usize::from(program.slot) >= SLOT_COUNT {
            return invalid(format!(
                "F108 macro {} uses slot {}; maximum slot is {}",
                program.id,
                program.slot,
                SLOT_COUNT - 1
            ));
        }
        if !slots.insert(program.slot) {
            return invalid(format!("duplicate F108 macro slot: {}", program.slot));
        }
        if program.events.len() > usize::from(u16::MAX) {
            return invalid(format!(
                "F108 macro slot {slot} has {} events; maximum is {}",
                program.events.len(),
                u16::MAX
            ));
        }
        total_events = total_events
            .checked_add(program.events.len())
            .ok_or_else(|| {
                DriverError::InvalidConfiguration("macro event count overflow".into())
            })?;
        for (event_index, event) in program.events.iter().enumerate() {
            validate_event(slot, event_index, event)?;
        }
    }

    let safety_score = programs
        .len()
        .checked_add(total_events)
        .and_then(|count| count.checked_mul(8))
        .ok_or_else(|| DriverError::InvalidConfiguration("macro bank size overflow".into()))?;
    if safety_score > USB_SAFETY_SCORE_LIMIT {
        return invalid(format!(
            "F108 USB macro safety score is {safety_score} bytes; maximum is {USB_SAFETY_SCORE_LIMIT}"
        ));
    }

    let mut data = vec![0xFF; DIRECTORY_SIZE];
    for program in programs {
        if program.events.is_empty() {
            continue;
        }
        let offset = data.len();
        let offset_u16 = u16::try_from(offset).map_err(|_| {
            DriverError::InvalidConfiguration("F108 macro data offset exceeds 16 bits".into())
        })?;
        let directory_offset = usize::from(program.slot) * 4;
        data[directory_offset..directory_offset + 4].copy_from_slice(&[
            offset_u16 as u8,
            (offset_u16 >> 8) as u8,
            0,
            0,
        ]);

        let event_count = program.events.len() as u16;
        data.extend_from_slice(&event_count.to_le_bytes());
        data.extend_from_slice(&[0; 6]);
        for event in &program.events {
            data.extend_from_slice(&encode_event(event)?);
        }
    }

    let padded_size = (data.len() + 2).div_ceil(FEATURE_REPORT_SIZE) * FEATURE_REPORT_SIZE;
    if padded_size > FORMAT_MAX_BYTES {
        return invalid(format!(
            "serialized F108 macro bank is {padded_size} bytes; format maximum is {FORMAT_MAX_BYTES}"
        ));
    }
    data.resize(padded_size, 0);
    data[padded_size - 2] = 0x55;
    data[padded_size - 1] = 0xAA;
    let packet_count = u8::try_from(padded_size / FEATURE_REPORT_SIZE).map_err(|_| {
        DriverError::InvalidConfiguration("F108 macro packet count exceeds one byte".into())
    })?;

    Ok(SerializedMacroBank {
        payload: data,
        packet_count,
    })
}

pub fn send_bank(
    session: &mut AulaF108ProSession,
    context: &OperationContext,
    programs: &[HardwareMacroProgram],
) -> Result<(), DriverError> {
    // Safety invariant: no packet is sent until the entire bank has serialized.
    let bank = serialize_bank(programs)?;

    session.send(context, command(MACRO_INIT_COMMAND), true)?;
    let mut header = command(DATA_HEADER_COMMAND);
    header[8] = bank.packet_count;
    session.send(context, header, true)?;
    send_payload(session, context, &bank.payload)?;
    session.send(context, command(APPLY_COMMAND), true)
}

fn command(command: u8) -> [u8; FEATURE_REPORT_SIZE] {
    let mut report = [0; FEATURE_REPORT_SIZE];
    report[0] = 0x04;
    report[1] = command;
    report
}

fn send_payload(
    session: &AulaF108ProSession,
    context: &OperationContext,
    payload: &[u8],
) -> Result<(), DriverError> {
    for chunk in payload.chunks_exact(FEATURE_REPORT_SIZE) {
        context.check()?;
        let mut report = [0; FEATURE_REPORT_SIZE];
        report.copy_from_slice(chunk);
        session
            .transport
            .set_feature_report(&report)
            .map_err(map_transport_error)?;
        context.sleep(COMMAND_DELAY)?;
    }
    let response = session
        .transport
        .get_feature_report()
        .map_err(map_transport_error)?;
    validate_ack(&response)?;
    context.sleep(COMMAND_DELAY)
}

fn validate_event(
    slot: usize,
    event_index: usize,
    event: &HardwareMacroEvent,
) -> Result<(), DriverError> {
    match event {
        HardwareMacroEvent::KeyDown { usage } | HardwareMacroEvent::KeyUp { usage } => {
            if *usage == 0 || *usage > u16::from(u8::MAX) {
                return invalid(format!(
                    "F108 macro slot {slot} event {event_index} has unsupported HID usage 0x{usage:04X}"
                ));
            }
        }
        HardwareMacroEvent::Delay { milliseconds } if *milliseconds > MAX_DELAY_MS => {
            return invalid(format!(
                "F108 macro slot {slot} event {event_index} delay is {milliseconds} ms; maximum is {MAX_DELAY_MS} ms"
            ));
        }
        HardwareMacroEvent::MouseDown { button } | HardwareMacroEvent::MouseUp { button }
            if matches!(
                button,
                HardwareMacroMouseButton::Button4 | HardwareMacroMouseButton::Button5
            ) =>
        {
            return Err(DriverError::UnsupportedCapability(
                "F108 macro wire codes are documented only for left, right, and middle buttons",
            ));
        }
        _ => {}
    }
    Ok(())
}

fn encode_event(event: &HardwareMacroEvent) -> Result<[u8; 4], DriverError> {
    match event {
        HardwareMacroEvent::KeyDown { usage } => Ok([0, 0, *usage as u8, 0xB0]),
        HardwareMacroEvent::KeyUp { usage } => Ok([0, 0, *usage as u8, 0x30]),
        HardwareMacroEvent::MouseDown { button } => Ok([0, 0, mouse_code(*button)?, 0x90]),
        HardwareMacroEvent::MouseUp { button } => Ok([0, 0, mouse_code(*button)?, 0x10]),
        HardwareMacroEvent::Delay { milliseconds } => {
            let delay = (*milliseconds).max(MIN_DELAY_MS) as u16;
            let [low, high] = delay.to_le_bytes();
            Ok([low, high, 0, 0x50])
        }
    }
}

fn mouse_code(button: HardwareMacroMouseButton) -> Result<u8, DriverError> {
    match button {
        HardwareMacroMouseButton::Left => Ok(0x01),
        HardwareMacroMouseButton::Right => Ok(0x04),
        HardwareMacroMouseButton::Middle => Ok(0x02),
        HardwareMacroMouseButton::Button4 | HardwareMacroMouseButton::Button5 => Err(
            DriverError::UnsupportedCapability("unsupported F108 macro mouse button"),
        ),
    }
}

fn invalid<T>(message: String) -> Result<T, DriverError> {
    Err(DriverError::InvalidConfiguration(message))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn program(id: &str, events: Vec<HardwareMacroEvent>) -> HardwareMacroProgram {
        HardwareMacroProgram {
            id: id.into(),
            slot: if id == "empty" { 1 } else { 0 },
            events,
        }
    }

    #[test]
    fn serializes_directory_events_padding_and_trailer() {
        let bank = serialize_bank(&[
            program(
                "one",
                vec![
                    HardwareMacroEvent::KeyDown { usage: 0x04 },
                    HardwareMacroEvent::Delay { milliseconds: 9 },
                    HardwareMacroEvent::KeyUp { usage: 0x04 },
                ],
            ),
            program("empty", vec![]),
        ])
        .unwrap();

        assert_eq!(&bank.payload[0..4], &[0x90, 0x01, 0, 0]);
        assert_eq!(&bank.payload[4..8], &[0xFF; 4]);
        assert_eq!(&bank.payload[400..408], &[3, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(&bank.payload[408..412], &[0, 0, 0x04, 0xB0]);
        assert_eq!(&bank.payload[412..416], &[10, 0, 0, 0x50]);
        assert_eq!(&bank.payload[416..420], &[0, 0, 0x04, 0x30]);
        assert_eq!(&bank.payload[bank.payload.len() - 2..], &[0x55, 0xAA]);
        assert_eq!(usize::from(bank.packet_count) * 64, bank.payload.len());
    }

    #[test]
    fn invalid_bank_is_rejected_before_transport_is_needed() {
        let oversized = vec![program(
            "x",
            vec![HardwareMacroEvent::KeyDown { usage: 4 }; 338],
        )];
        assert!(matches!(
            serialize_bank(&oversized),
            Err(DriverError::InvalidConfiguration(_))
        ));

        let invalid_delay = [program(
            "delay",
            vec![HardwareMacroEvent::Delay {
                milliseconds: MAX_DELAY_MS + 1,
            }],
        )];
        assert!(serialize_bank(&invalid_delay).is_err());
    }

    #[test]
    fn empty_slots_are_ff_and_empty_bank_can_clear_hardware() {
        let bank = serialize_bank(&[]).unwrap();
        assert!(
            bank.payload[..DIRECTORY_SIZE]
                .iter()
                .all(|byte| *byte == 0xFF)
        );
        assert_eq!(&bank.payload[bank.payload.len() - 2..], &[0x55, 0xAA]);
    }
}
