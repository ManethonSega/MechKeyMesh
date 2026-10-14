use super::{AulaF108ProSession, FEATURE_REPORT_SIZE, FeatureReadback};
use mechkeymesh_driver_api::{
    Connection, DeviceClock, DriverError, LcdPixelFormat, LcdUpload, OperationContext,
};
use mechkeymesh_hid::{LCD_ACK_SIZE, LCD_PAGE_SIZE};
use std::time::Duration;

const WIDTH: u32 = 240;
const HEIGHT: u32 = 135;
const HEADER_BYTES: usize = 256;
const FRAME_BYTES: usize = WIDTH as usize * HEIGHT as usize * 2;
const MAX_FRAMES: usize = 141;
const ACK_TIMEOUT: Duration = Duration::from_millis(300);

pub(crate) fn upload(
    session: &mut AulaF108ProSession,
    context: &OperationContext,
    upload: &LcdUpload,
    progress: &mut dyn FnMut(usize, usize),
) -> Result<(), DriverError> {
    preflight(session, upload)?;
    context.check()?;

    let page_count = upload.payload.len() / LCD_PAGE_SIZE;
    let page_count_u16 = u16::try_from(page_count).map_err(|_| {
        DriverError::InvalidConfiguration("LCD page count exceeds the protocol field".into())
    })?;

    session.begin_transaction(context)?;
    let mut header = [0u8; FEATURE_REPORT_SIZE];
    header[0] = 0x04;
    header[1] = 0x72;
    header[2] = 0x01;
    header[8..10].copy_from_slice(&page_count_u16.to_le_bytes());
    session.send(context, header, true)?;

    let transport = session
        .lcd_transport
        .as_ref()
        .ok_or(DriverError::UnsupportedCapability("display"))?;
    for (index, page) in upload.payload.chunks_exact(LCD_PAGE_SIZE).enumerate() {
        context.check()?;
        let page: &[u8; LCD_PAGE_SIZE] = page
            .try_into()
            .map_err(|_| DriverError::Internal("validated LCD page changed length".into()))?;
        transport
            .write_page(page)
            .map_err(super::map_transport_error)?;
        let ack = transport
            .read_ack(ACK_TIMEOUT)
            .map_err(super::map_transport_error)?
            .ok_or_else(|| {
                DriverError::Timeout(format!(
                    "LCD page {}/{} acknowledgment timed out",
                    index + 1,
                    page_count
                ))
            })?;
        validate_page_ack(index, &ack)?;
        progress(index + 1, page_count);
    }

    context.check()?;
    session.apply_transaction(context)
}

pub(crate) fn sync_clock(
    session: &mut AulaF108ProSession,
    context: &OperationContext,
    clock: DeviceClock,
) -> Result<(), DriverError> {
    validate_clock(clock)?;
    if session.candidate.connection != Connection::Usb {
        return Err(DriverError::UnsupportedCapability(
            "clock over non-USB transport",
        ));
    }

    session.begin_transaction(context)?;
    let mut init = [0u8; FEATURE_REPORT_SIZE];
    init[0] = 0x04;
    init[1] = 0x28;
    init[8] = 0x01;
    session.send(context, init, true)?;

    let mut data = [0u8; FEATURE_REPORT_SIZE];
    data[1] = 0x01;
    data[2] = 0x5a;
    data[3] = (clock.year % 2000) as u8;
    data[4] = clock.month;
    data[5] = clock.day;
    data[6] = clock.hour;
    data[7] = clock.minute;
    data[8] = clock.second;
    data[10] = clock.weekday_sunday_zero;
    data[62] = 0x55;
    data[63] = 0xaa;
    session.send_with_readback(context, data, FeatureReadback::ClockData)?;
    session.apply_transaction(context)
}

pub(crate) fn validate_clock_readback(
    sent: &[u8; FEATURE_REPORT_SIZE],
    response: &[u8; FEATURE_REPORT_SIZE],
) -> Result<(), DriverError> {
    // The clock data report stores year - 2000 at payload byte 3. A complete
    // echo of the report therefore contains 0x1A for 2026, not ACK 0x01.
    // Match all 64 bytes before accepting an echo; a year byte alone is not
    // evidence that the readback belongs to this request.
    if response == sent {
        return Ok(());
    }
    // Retain support for the command-style ACK form used by existing firmware.
    super::validate_ack(response)
}

pub(crate) fn validate_upload(upload: &LcdUpload) -> Result<(), DriverError> {
    if upload.width != WIDTH || upload.height != HEIGHT {
        return invalid(format!(
            "LCD dimensions must be {WIDTH}x{HEIGHT}, got {}x{}",
            upload.width, upload.height
        ));
    }
    if upload.pixel_format != LcdPixelFormat::Rgb565LittleEndian {
        return invalid("LCD pixel format must be RGB565 little-endian".into());
    }
    if upload.frame_count == 0 || upload.frame_count > MAX_FRAMES {
        return invalid(format!(
            "LCD frame count {} is outside 1..={MAX_FRAMES}",
            upload.frame_count
        ));
    }
    if upload.header_bytes != HEADER_BYTES {
        return invalid(format!("LCD header must be exactly {HEADER_BYTES} bytes"));
    }
    let expected_content = HEADER_BYTES + upload.frame_count * FRAME_BYTES;
    if upload.content_length != expected_content {
        return invalid("LCD content length does not match frame count".into());
    }
    let expected_payload = expected_content.div_ceil(LCD_PAGE_SIZE) * LCD_PAGE_SIZE;
    if upload.payload.len() != expected_payload
        || !upload.payload.len().is_multiple_of(LCD_PAGE_SIZE)
    {
        return invalid("LCD payload length or page count is not exact".into());
    }
    if upload.payload[0] as usize != upload.frame_count {
        return invalid("LCD header frame count does not match metadata".into());
    }
    if upload.payload[1..=upload.frame_count].contains(&0) {
        return invalid("LCD header contains a zero frame delay".into());
    }
    if upload.payload[upload.frame_count + 1..HEADER_BYTES]
        .iter()
        .any(|byte| *byte != 0xff)
    {
        return invalid("LCD unused header bytes must be 0xFF".into());
    }
    Ok(())
}

fn preflight(session: &AulaF108ProSession, upload: &LcdUpload) -> Result<(), DriverError> {
    if session.candidate.connection != Connection::Usb
        || session.candidate.vendor_id != Some(super::VENDOR_ID)
        || session.candidate.product_id != Some(super::PRODUCT_ID)
    {
        return invalid("LCD upload requires the verified F108 Pro USB device".into());
    }
    if session.lcd_transport.is_none() {
        return Err(DriverError::UnsupportedCapability(
            "dedicated F108 LCD interface",
        ));
    }
    validate_upload(upload)
}

fn validate_page_ack(index: usize, ack: &[u8; LCD_ACK_SIZE]) -> Result<(), DriverError> {
    if ack[..4] != [0x01, 0x5a, 0x02, 0x00] {
        return Err(DriverError::Protocol(format!(
            "LCD page {} returned an unexpected acknowledgment",
            index + 1
        )));
    }
    Ok(())
}

fn validate_clock(clock: DeviceClock) -> Result<(), DriverError> {
    let valid = (2000..=2255).contains(&clock.year)
        && (1..=12).contains(&clock.month)
        && (1..=31).contains(&clock.day)
        && clock.hour <= 23
        && clock.minute <= 59
        && clock.second <= 59
        && clock.weekday_sunday_zero <= 6;
    if valid {
        Ok(())
    } else {
        invalid("clock fields are outside the verified F108 ranges".into())
    }
}

fn invalid<T>(message: String) -> Result<T, DriverError> {
    Err(DriverError::InvalidConfiguration(message))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mechkeymesh_driver_api::DeviceCandidate;
    use mechkeymesh_hid::{FeatureTransport, HidTransportError};
    use std::sync::{Arc, Mutex};

    type CapturedReports = Arc<Mutex<Vec<[u8; FEATURE_REPORT_SIZE]>>>;

    struct ClockTransport {
        reports: CapturedReports,
        corrupt_echo: bool,
    }

    impl FeatureTransport for ClockTransport {
        fn set_feature_report(
            &self,
            report: &[u8; FEATURE_REPORT_SIZE],
        ) -> Result<(), HidTransportError> {
            self.reports.lock().unwrap().push(*report);
            Ok(())
        }

        fn get_feature_report(&self) -> Result<[u8; FEATURE_REPORT_SIZE], HidTransportError> {
            let mut response = *self.reports.lock().unwrap().last().unwrap();
            if response[0] == 0x04 {
                response[3] = 0x01;
            } else if self.corrupt_echo {
                response[7] ^= 1;
            }
            Ok(response)
        }
    }

    fn clock_session(corrupt_echo: bool) -> (AulaF108ProSession, CapturedReports) {
        let reports = Arc::new(Mutex::new(Vec::new()));
        let session = AulaF108ProSession {
            candidate: DeviceCandidate {
                driver_id: super::super::DRIVER_ID.into(),
                instance_id: "test-clock".into(),
                name: "AULA F108 Pro".into(),
                connection: Connection::Usb,
                vendor_id: Some(super::super::VENDOR_ID),
                product_id: Some(super::super::PRODUCT_ID),
                usage_page: Some(super::super::CONFIG_USAGE_PAGE),
                interface_number: Some(3),
                serial_number: None,
            },
            transport: Box::new(ClockTransport {
                reports: Arc::clone(&reports),
                corrupt_echo,
            }),
            lcd_transport: None,
        };
        (session, reports)
    }

    fn clock() -> DeviceClock {
        DeviceClock {
            year: 2026,
            month: 10,
            day: 13,
            hour: 13,
            minute: 25,
            second: 15,
            weekday_sunday_zero: 2,
        }
    }

    #[test]
    fn clock_echo_with_year_0x1a_reaches_apply() {
        let (mut session, reports) = clock_session(false);
        sync_clock(
            &mut session,
            &OperationContext::new(Duration::from_secs(2)),
            clock(),
        )
        .unwrap();
        let reports = reports.lock().unwrap();
        assert_eq!(reports.len(), 4);
        assert_eq!(&reports[0][..2], &[0x04, 0x18]);
        assert_eq!(&reports[1][..2], &[0x04, 0x28]);
        assert_eq!(&reports[2][..9], &[0, 1, 0x5a, 0x1a, 10, 13, 13, 25, 15]);
        assert_eq!(&reports[3][..2], &[0x04, 0x02]);
        // The command ACK checker must not globally accept the year byte.
        assert!(super::super::validate_ack(&reports[2]).is_err());
    }

    #[test]
    fn mismatched_clock_echo_fails_before_apply() {
        let (mut session, reports) = clock_session(true);
        let error = sync_clock(
            &mut session,
            &OperationContext::new(Duration::from_secs(2)),
            clock(),
        )
        .unwrap_err();
        assert_eq!(reports.lock().unwrap().len(), 3);
        assert!(error.to_string().contains("sent [00, 01, 5A, 1A"));
        assert!(error.to_string().contains("received [00, 01, 5A, 1A"));
    }

    #[test]
    fn clock_path_preserves_ack_and_rejects_unrelated_data() {
        let mut sent = [0u8; FEATURE_REPORT_SIZE];
        sent[..4].copy_from_slice(&[0, 1, 0x5a, 0x1a]);
        let mut ack = [0u8; FEATURE_REPORT_SIZE];
        ack[..4].copy_from_slice(&[0x04, 0x28, 0, 1]);
        assert!(validate_clock_readback(&sent, &ack).is_ok());
        assert!(validate_clock_readback(&sent, &[0xff; FEATURE_REPORT_SIZE]).is_err());
        assert!(validate_clock_readback(&sent, &[0; FEATURE_REPORT_SIZE]).is_err());
    }

    fn valid_upload(frames: usize) -> LcdUpload {
        let content_length = HEADER_BYTES + frames * FRAME_BYTES;
        let mut payload = vec![0xff; content_length.div_ceil(LCD_PAGE_SIZE) * LCD_PAGE_SIZE];
        payload[0] = frames as u8;
        payload[1..=frames].fill(1);
        LcdUpload {
            width: WIDTH,
            height: HEIGHT,
            pixel_format: LcdPixelFormat::Rgb565LittleEndian,
            frame_count: frames,
            header_bytes: HEADER_BYTES,
            content_length,
            payload,
        }
    }

    #[test]
    fn accepts_exact_141_frame_asset() {
        assert!(validate_upload(&valid_upload(141)).is_ok());
    }

    #[test]
    fn blocks_frame_142_before_transport() {
        assert!(validate_upload(&valid_upload(142)).is_err());
    }

    #[test]
    fn blocks_mismatched_page_count_and_header() {
        let mut upload = valid_upload(1);
        upload.payload.extend_from_slice(&[0; LCD_PAGE_SIZE]);
        assert!(validate_upload(&upload).is_err());
        let mut upload = valid_upload(1);
        upload.payload[0] = 2;
        assert!(validate_upload(&upload).is_err());
    }
}
