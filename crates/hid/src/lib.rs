use thiserror::Error;

pub const FEATURE_REPORT_SIZE: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HidDeviceInfo {
    pub path: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub usage_page: u16,
    pub interface_number: i32,
    pub serial_number: Option<String>,
}

#[derive(Debug, Error)]
pub enum HidTransportError {
    #[error("device not found")]
    NotFound,
    #[error("device disconnected: {0}")]
    Disconnected(String),
    #[error("device access denied: {0}")]
    PermissionDenied(String),
    #[error("device is busy: {0}")]
    Busy(String),
    #[error("HID transport is not supported on this platform")]
    UnsupportedPlatform,
    #[error("{0}")]
    Backend(String),
}

pub trait FeatureTransport: Send {
    fn set_feature_report(
        &self,
        report: &[u8; FEATURE_REPORT_SIZE],
    ) -> Result<(), HidTransportError>;

    fn get_feature_report(&self) -> Result<[u8; FEATURE_REPORT_SIZE], HidTransportError>;

    fn get_feature_report_prefix(
        &self,
        payload_length: usize,
    ) -> Result<[u8; FEATURE_REPORT_SIZE], HidTransportError> {
        if payload_length == 0 || payload_length > FEATURE_REPORT_SIZE {
            return Err(HidTransportError::Backend(format!(
                "feature-report prefix length {payload_length} is outside 1..={FEATURE_REPORT_SIZE}"
            )));
        }
        self.get_feature_report()
    }
}

pub fn classify_backend_error(message: impl Into<String>) -> HidTransportError {
    let message = message.into();
    let lower = message.to_ascii_lowercase();

    if lower.contains("access is denied")
        || lower.contains("access denied")
        || lower.contains("permission denied")
    {
        HidTransportError::PermissionDenied(message)
    } else if lower.contains("device is not connected")
        || lower.contains("not connected")
        || lower.contains("disconnected")
        || lower.contains("no such device")
        || lower.contains("device has been removed")
    {
        HidTransportError::Disconnected(message)
    } else if lower.contains("device is busy")
        || lower.contains("resource busy")
        || lower.contains("being used")
        || lower.contains("cannot access the file because it is being used")
    {
        HidTransportError::Busy(message)
    } else {
        HidTransportError::Backend(message)
    }
}

#[cfg(windows)]
mod windows {
    use super::{
        FEATURE_REPORT_SIZE, FeatureTransport, HidDeviceInfo, HidTransportError,
        classify_backend_error,
    };
    use hidapi::{HidApi, HidDevice};
    use std::ffi::CString;

    pub struct WindowsFeatureTransport {
        device: HidDevice,
    }

    impl FeatureTransport for WindowsFeatureTransport {
        fn set_feature_report(
            &self,
            report: &[u8; FEATURE_REPORT_SIZE],
        ) -> Result<(), HidTransportError> {
            let mut buffer = [0u8; FEATURE_REPORT_SIZE + 1];
            buffer[1..].copy_from_slice(report);
            self.device
                .send_feature_report(&buffer)
                .map_err(|error| classify_backend_error(error.to_string()))?;
            Ok(())
        }

        fn get_feature_report(&self) -> Result<[u8; FEATURE_REPORT_SIZE], HidTransportError> {
            let mut buffer = [0u8; FEATURE_REPORT_SIZE + 1];
            self.device
                .get_feature_report(&mut buffer)
                .map_err(|error| classify_backend_error(error.to_string()))?;

            let mut report = [0u8; FEATURE_REPORT_SIZE];
            report.copy_from_slice(&buffer[1..]);
            Ok(report)
        }

        fn get_feature_report_prefix(
            &self,
            payload_length: usize,
        ) -> Result<[u8; FEATURE_REPORT_SIZE], HidTransportError> {
            if payload_length == 0 || payload_length > FEATURE_REPORT_SIZE {
                return Err(HidTransportError::Backend(format!(
                    "feature-report prefix length {payload_length} is outside 1..={FEATURE_REPORT_SIZE}"
                )));
            }

            // HIDAPI includes the report-ID byte in the buffer length.
            // The F108 OEM 04 F5 reader requests a 9-byte total report for
            // its initial ACK, which is report ID + 8 payload bytes.
            let mut buffer = vec![0u8; payload_length + 1];
            self.device
                .get_feature_report(&mut buffer)
                .map_err(|error| classify_backend_error(error.to_string()))?;

            let mut report = [0u8; FEATURE_REPORT_SIZE];
            report[..payload_length].copy_from_slice(&buffer[1..]);
            Ok(report)
        }
    }

    pub fn enumerate_feature_devices(
        vendor_id: u16,
        product_id: u16,
        usage_page: u16,
    ) -> Result<Vec<HidDeviceInfo>, HidTransportError> {
        let api = HidApi::new().map_err(|error| classify_backend_error(error.to_string()))?;

        let devices = api
            .device_list()
            .filter(|device| {
                device.vendor_id() == vendor_id
                    && device.product_id() == product_id
                    && device.usage_page() == usage_page
            })
            .map(|device| HidDeviceInfo {
                path: device.path().to_string_lossy().into_owned(),
                vendor_id: device.vendor_id(),
                product_id: device.product_id(),
                usage_page: device.usage_page(),
                interface_number: device.interface_number(),
                serial_number: device.serial_number().map(ToOwned::to_owned),
            })
            .collect();

        Ok(devices)
    }

    pub fn open_feature_path(path: &str) -> Result<Box<dyn FeatureTransport>, HidTransportError> {
        let api = HidApi::new().map_err(|error| classify_backend_error(error.to_string()))?;
        let path = CString::new(path)
            .map_err(|_| HidTransportError::Backend("HID path contains an interior NUL".into()))?;
        let device = api
            .open_path(&path)
            .map_err(|error| classify_backend_error(error.to_string()))?;

        Ok(Box::new(WindowsFeatureTransport { device }))
    }
}

#[cfg(windows)]
pub use windows::{enumerate_feature_devices, open_feature_path};

#[cfg(not(windows))]
pub fn enumerate_feature_devices(
    _vendor_id: u16,
    _product_id: u16,
    _usage_page: u16,
) -> Result<Vec<HidDeviceInfo>, HidTransportError> {
    Err(HidTransportError::UnsupportedPlatform)
}

#[cfg(not(windows))]
pub fn open_feature_path(_path: &str) -> Result<Box<dyn FeatureTransport>, HidTransportError> {
    Err(HidTransportError::UnsupportedPlatform)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feature_report_prefix_rejects_invalid_lengths_by_default() {
        struct Fake;

        impl FeatureTransport for Fake {
            fn set_feature_report(
                &self,
                _report: &[u8; FEATURE_REPORT_SIZE],
            ) -> Result<(), HidTransportError> {
                Ok(())
            }

            fn get_feature_report(&self) -> Result<[u8; FEATURE_REPORT_SIZE], HidTransportError> {
                Ok([0u8; FEATURE_REPORT_SIZE])
            }
        }

        let fake = Fake;
        assert!(fake.get_feature_report_prefix(1).is_ok());
        assert!(fake.get_feature_report_prefix(FEATURE_REPORT_SIZE).is_ok());
        assert!(fake.get_feature_report_prefix(0).is_err());
        assert!(
            fake.get_feature_report_prefix(FEATURE_REPORT_SIZE + 1)
                .is_err()
        );
    }

    #[test]
    fn classifies_common_windows_errors() {
        assert!(matches!(
            classify_backend_error("Access is denied."),
            HidTransportError::PermissionDenied(_)
        ));
        assert!(matches!(
            classify_backend_error("The device is not connected."),
            HidTransportError::Disconnected(_)
        ));
        assert!(matches!(
            classify_backend_error("The device is busy"),
            HidTransportError::Busy(_)
        ));
        assert!(matches!(
            classify_backend_error("unexpected HID failure"),
            HidTransportError::Backend(_)
        ));
    }
}
