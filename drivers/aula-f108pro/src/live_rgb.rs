use super::{AulaF108ProSession, map_transport_error, validate_ack};
use mechkeymesh_driver_api::{DriverError, LiveRgbFrame, OperationContext};
use mechkeymesh_hid::FEATURE_REPORT_SIZE;
use std::collections::HashSet;

pub const MAXIMUM_FRAMES_PER_SECOND: u16 = 30;
pub const MAXIMUM_LEDS: usize = 112;
const DATA_REPORTS: usize = 7;
const RECORD_SIZE: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveRgbTransfer {
    pub start: [u8; FEATURE_REPORT_SIZE],
    pub data: [[u8; FEATURE_REPORT_SIZE]; DATA_REPORTS],
    pub clear: [[u8; FEATURE_REPORT_SIZE]; 2],
    pub apply: [u8; FEATURE_REPORT_SIZE],
}

pub fn build_transfer(frame: &LiveRgbFrame) -> Result<LiveRgbTransfer, DriverError> {
    if frame.colors.len() > MAXIMUM_LEDS {
        return Err(DriverError::InvalidConfiguration(format!(
            "live RGB frame has {} LEDs; maximum is {MAXIMUM_LEDS}",
            frame.colors.len()
        )));
    }

    let mut colors = frame.colors.clone();
    colors.sort_by_key(|assignment| assignment.led_index);
    let mut seen = HashSet::new();
    let mut data = [[0u8; FEATURE_REPORT_SIZE]; DATA_REPORTS];
    for (position, assignment) in colors.iter().enumerate() {
        if assignment.led_index == 0 || assignment.led_index > u16::from(u8::MAX) {
            return Err(DriverError::InvalidConfiguration(format!(
                "live RGB LED index {} is outside 1..=255",
                assignment.led_index
            )));
        }
        if !seen.insert(assignment.led_index) {
            return Err(DriverError::InvalidConfiguration(format!(
                "duplicate live RGB LED index {}",
                assignment.led_index
            )));
        }
        let byte = position * RECORD_SIZE;
        let report = byte / FEATURE_REPORT_SIZE;
        let offset = byte % FEATURE_REPORT_SIZE;
        data[report][offset] = assignment.led_index as u8;
        data[report][offset + 1] = assignment.color.r;
        data[report][offset + 2] = assignment.color.g;
        data[report][offset + 3] = assignment.color.b;
    }

    let mut start = [0u8; FEATURE_REPORT_SIZE];
    start[0] = 0x04;
    start[1] = 0x20;
    start[8] = 0x08;

    let mut apply = [0u8; FEATURE_REPORT_SIZE];
    apply[0] = 0x04;
    apply[1] = 0x02;

    Ok(LiveRgbTransfer {
        start,
        data,
        clear: [[0u8; FEATURE_REPORT_SIZE]; 2],
        apply,
    })
}

pub fn send_frame(
    session: &AulaF108ProSession,
    context: &OperationContext,
    frame: &LiveRgbFrame,
) -> Result<(), DriverError> {
    // Build and validate every report before the first HID write. The 04 20
    // family is volatile preview streaming and does not rewrite stored RGB.
    let transfer = build_transfer(frame)?;
    send_with_ack(session, context, &transfer.start)?;
    for report in &transfer.data {
        send(session, context, report)?;
    }
    for report in &transfer.clear {
        send(session, context, report)?;
    }
    send_with_ack(session, context, &transfer.apply)
}

fn send(
    session: &AulaF108ProSession,
    context: &OperationContext,
    report: &[u8; FEATURE_REPORT_SIZE],
) -> Result<(), DriverError> {
    context.check()?;
    session
        .transport
        .set_feature_report(report)
        .map_err(map_transport_error)
}

fn send_with_ack(
    session: &AulaF108ProSession,
    context: &OperationContext,
    report: &[u8; FEATURE_REPORT_SIZE],
) -> Result<(), DriverError> {
    send(session, context, report)?;
    context.check()?;
    let response = session
        .transport
        .get_feature_report()
        .map_err(map_transport_error)?;
    validate_ack(&response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mechkeymesh_driver_api::{PerKeyRgbAssignment, RgbColor};

    fn color(led_index: u16, r: u8, g: u8, b: u8) -> PerKeyRgbAssignment {
        PerKeyRgbAssignment {
            led_index,
            color: RgbColor { r, g, b },
        }
    }

    #[test]
    fn serializes_complete_sorted_frame_before_transport() {
        let transfer = build_transfer(&LiveRgbFrame {
            colors: vec![color(19, 1, 2, 3), color(1, 4, 5, 6)],
        })
        .unwrap();
        assert_eq!(&transfer.start[..9], &[0x04, 0x20, 0, 0, 0, 0, 0, 0, 0x08]);
        assert_eq!(&transfer.data[0][..8], &[1, 4, 5, 6, 19, 1, 2, 3]);
        assert!(transfer.data[1..].iter().flatten().all(|byte| *byte == 0));
        assert_eq!(&transfer.apply[..2], &[0x04, 0x02]);
    }

    #[test]
    fn rejects_invalid_duplicate_or_oversized_frames() {
        assert!(
            build_transfer(&LiveRgbFrame {
                colors: vec![color(0, 1, 2, 3)]
            })
            .is_err()
        );
        assert!(
            build_transfer(&LiveRgbFrame {
                colors: vec![color(1, 1, 2, 3), color(1, 4, 5, 6)]
            })
            .is_err()
        );
        assert!(
            build_transfer(&LiveRgbFrame {
                colors: (1..=113).map(|index| color(index, 1, 2, 3)).collect()
            })
            .is_err()
        );
    }
}
