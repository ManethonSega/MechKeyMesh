use super::{AulaF108ProSession, COMMAND_DELAY, map_transport_error};
use mechkeymesh_driver_api::{
    DriverError, OperationContext, PerKeyRgbAssignment, PerKeyRgbCapabilities, PerKeyRgbConfig,
    PerKeyRgbReadback, RgbColor,
};
use mechkeymesh_hid::FEATURE_REPORT_SIZE;
use std::collections::HashSet;

const PER_KEY_RGB_COMMAND: u8 = 0x23;
const PER_KEY_READBACK_COMMAND: u8 = 0xF5;
const RGB_MODE: u8 = 0x09;
const PER_KEY_TABLE_SIZE: usize = 0x240;
const TRAILER_OFFSET: usize = PER_KEY_TABLE_SIZE - 2;

pub fn capabilities() -> PerKeyRgbCapabilities {
    PerKeyRgbCapabilities {
        brightness_min: 0,
        brightness_max: 5,
        readback_supported: true,
    }
}

pub fn validate_config(config: &PerKeyRgbConfig) -> Result<(), DriverError> {
    if config.brightness > 5 {
        return Err(DriverError::InvalidConfiguration(
            "per-key RGB brightness must be between 0 and 5".into(),
        ));
    }

    let mut seen = HashSet::new();
    for assignment in &config.colors {
        validate_led_index(assignment.led_index)?;
        if !seen.insert(assignment.led_index) {
            return Err(DriverError::InvalidConfiguration(format!(
                "duplicate F108 LED index {}",
                assignment.led_index
            )));
        }
    }
    Ok(())
}

pub fn build_rgb_table(
    config: &PerKeyRgbConfig,
) -> Result<[u8; PER_KEY_TABLE_SIZE], DriverError> {
    validate_config(config)?;
    let mut table = [0u8; PER_KEY_TABLE_SIZE];

    for assignment in &config.colors {
        let offset = usize::from(assignment.led_index) * 4;
        table[offset] = assignment.led_index as u8;
        table[offset + 1] = assignment.color.r;
        table[offset + 2] = assignment.color.g;
        table[offset + 3] = assignment.color.b;
    }

    // Exact OEM DeviceDriver.exe writes uint16 0x55AA at offset 0x23E.
    // On little-endian x86 that is AA 55 on the wire.
    table[TRAILER_OFFSET] = 0xAA;
    table[TRAILER_OFFSET + 1] = 0x55;
    Ok(table)
}

pub fn brightness_preamble_payload(
    brightness: u8,
) -> Result<[u8; FEATURE_REPORT_SIZE], DriverError> {
    if brightness > 5 {
        return Err(DriverError::InvalidConfiguration(
            "per-key RGB brightness must be between 0 and 5".into(),
        ));
    }

    let mut data = [0u8; FEATURE_REPORT_SIZE];
    data[0] = 0x80;
    data[9] = brightness;
    // Exact OEM DeviceDriver.exe writes uint16 0x55AA at offsets 14-15.
    data[14] = 0xAA;
    data[15] = 0x55;
    Ok(data)
}

pub fn send_per_key_rgb(
    session: &mut AulaF108ProSession,
    context: &OperationContext,
    config: &PerKeyRgbConfig,
) -> Result<(), DriverError> {
    let table = build_rgb_table(config)?;

    // OEM FUN_0044b800 setup used immediately before stored per-key RGB.
    session.begin_transaction(context)?;
    session.lighting_init(context)?;
    session.send(
        context,
        brightness_preamble_payload(config.brightness)?,
        false,
    )?;
    session.apply_transaction(context)?;

    let mut preamble_finalize = [0u8; FEATURE_REPORT_SIZE];
    preamble_finalize[0] = 0x04;
    preamble_finalize[1] = 0xF0;
    session.send(context, preamble_finalize, false)?;

    // OEM FUN_0044b910 RGB table transaction.
    session.begin_transaction(context)?;
    let mut init = [0u8; FEATURE_REPORT_SIZE];
    init[0] = 0x04;
    init[1] = PER_KEY_RGB_COMMAND;
    init[8] = RGB_MODE;
    session.send(context, init, true)?;

    send_multi_packet(session, context, &table, true)?;
    session.apply_transaction(context)?;

    let mut finalize = [0u8; FEATURE_REPORT_SIZE];
    finalize[0] = 0x04;
    finalize[1] = 0xF0;
    session.send(context, finalize, true)?;
    Ok(())
}

pub fn read_per_key_rgb(
    session: &mut AulaF108ProSession,
    context: &OperationContext,
    led_indices: &[u16],
) -> Result<PerKeyRgbReadback, DriverError> {
    let mut unique = HashSet::new();
    for &index in led_indices {
        validate_led_index(index)?;
        if !unique.insert(index) {
            return Err(DriverError::InvalidConfiguration(format!(
                "duplicate requested F108 LED index {index}"
            )));
        }
    }

    // Exact OEM FUN_0042ae20 sends 04 F5 with byte 8 = 09 for RGB,
    // validates the first ACK, then reads nine 64-byte feature reports.
    let mut request = [0u8; FEATURE_REPORT_SIZE];
    request[0] = 0x04;
    request[1] = PER_KEY_READBACK_COMMAND;
    request[8] = RGB_MODE;

    context.check()?;
    session
        .transport
        .set_feature_report(&request)
        .map_err(map_transport_error)?;
    context.sleep(COMMAND_DELAY)?;

    // OEM FUN_0044f290 asks Windows for a 9-byte total HID feature
    // report here: one report-ID byte plus eight payload bytes.
    // That is sufficient to validate ACK payload byte 3 without consuming
    // the following full 64-byte RGB data reports.
    let ack = session
        .transport
        .get_feature_report_prefix(8)
        .map_err(map_transport_error)?;
    super::validate_ack(&ack)?;
    context.sleep(COMMAND_DELAY)?;

    let mut table = [0u8; PER_KEY_TABLE_SIZE];
    for (packet_index, chunk) in table.chunks_exact_mut(FEATURE_REPORT_SIZE).enumerate() {
        context.check()?;
        let report = session
            .transport
            .get_feature_report()
            .map_err(map_transport_error)?;
        chunk.copy_from_slice(&report);
        context.sleep(COMMAND_DELAY)?;

        if packet_index == 8 {
            break;
        }
    }

    // OEM readback handler closes with 04 02 and no 04 F0.
    session.apply_transaction(context)?;

    Ok(parse_rgb_readback(&table, led_indices)?)
}

pub fn parse_rgb_readback(
    table: &[u8; PER_KEY_TABLE_SIZE],
    led_indices: &[u16],
) -> Result<PerKeyRgbReadback, DriverError> {
    let mut colors = Vec::with_capacity(led_indices.len());
    for &led_index in led_indices {
        validate_led_index(led_index)?;
        let offset = usize::from(led_index) * 4;
        colors.push(PerKeyRgbAssignment {
            led_index,
            color: RgbColor {
                r: table[offset + 1],
                g: table[offset + 2],
                b: table[offset + 3],
            },
        });
    }
    colors.sort_by_key(|assignment| assignment.led_index);
    Ok(PerKeyRgbReadback { colors })
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
            .map_err(map_transport_error)?;
        context.sleep(COMMAND_DELAY)?;
    }

    if readback {
        let response = session
            .transport
            .get_feature_report()
            .map_err(map_transport_error)?;
        super::validate_ack(&response)?;
        context.sleep(COMMAND_DELAY)?;
    }
    Ok(())
}

fn validate_led_index(index: u16) -> Result<(), DriverError> {
    let offset = usize::from(index) * 4;
    if index == 0 || index > u16::from(u8::MAX) || offset + 4 > TRAILER_OFFSET {
        return Err(DriverError::InvalidConfiguration(format!(
            "F108 LED index {index} is outside the stored RGB table"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assignment(index: u16, r: u8, g: u8, b: u8) -> PerKeyRgbAssignment {
        PerKeyRgbAssignment {
            led_index: index,
            color: RgbColor { r, g, b },
        }
    }

    #[test]
    fn builds_oem_rgb_slot_format_and_exact_trailer_byte_order() {
        let table = build_rgb_table(&PerKeyRgbConfig {
            brightness: 5,
            colors: vec![
                assignment(1, 255, 0, 0),
                assignment(56, 0, 255, 0),
            ],
        })
        .unwrap();

        assert_eq!(&table[4..8], &[1, 255, 0, 0]);
        assert_eq!(&table[56 * 4..56 * 4 + 4], &[56, 0, 255, 0]);
        assert_eq!(&table[574..576], &[0xAA, 0x55]);
    }

    #[test]
    fn brightness_preamble_matches_exact_oem_offsets() {
        let payload = brightness_preamble_payload(4).unwrap();
        assert_eq!(payload[0], 0x80);
        assert_eq!(payload[9], 4);
        assert_eq!(&payload[14..16], &[0xAA, 0x55]);
        assert!(brightness_preamble_payload(6).is_err());
    }

    #[test]
    fn rejects_duplicate_and_invalid_led_indices() {
        assert!(
            build_rgb_table(&PerKeyRgbConfig {
                brightness: 5,
                colors: vec![
                    assignment(1, 1, 2, 3),
                    assignment(1, 4, 5, 6),
                ],
            })
            .is_err()
        );
        assert!(
            build_rgb_table(&PerKeyRgbConfig {
                brightness: 5,
                colors: vec![assignment(0, 1, 2, 3)],
            })
            .is_err()
        );
        assert!(
            build_rgb_table(&PerKeyRgbConfig {
                brightness: 6,
                colors: vec![],
            })
            .is_err()
        );
    }

    #[test]
    fn parses_f5_rgb_slots_using_led_index_offsets() {
        let mut table = [0u8; PER_KEY_TABLE_SIZE];
        table[1 * 4] = 1;
        table[1 * 4 + 1] = 10;
        table[1 * 4 + 2] = 20;
        table[1 * 4 + 3] = 30;
        table[56 * 4] = 56;
        table[56 * 4 + 1] = 40;
        table[56 * 4 + 2] = 50;
        table[56 * 4 + 3] = 60;

        let readback = parse_rgb_readback(&table, &[56, 1]).unwrap();
        assert_eq!(
            readback.colors,
            vec![
                assignment(1, 10, 20, 30),
                assignment(56, 40, 50, 60),
            ]
        );
    }

    #[test]
    fn capability_reports_f5_readback_support() {
        let capabilities = capabilities();
        assert_eq!(capabilities.brightness_min, 0);
        assert_eq!(capabilities.brightness_max, 5);
        assert!(capabilities.readback_supported);
    }
}
