use thiserror::Error;

pub const FEATURE_REPORT_SIZE: usize = 64;

#[derive(Debug, Error)]
pub enum HidTransportError {
    #[error("device not found")]
    NotFound,
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

    fn get_feature_report(
        &self,
    ) -> Result<[u8; FEATURE_REPORT_SIZE], HidTransportError>;
}

#[cfg(windows)]
mod windows {
    use super::{FeatureTransport, HidTransportError, FEATURE_REPORT_SIZE};
    use hidapi::{HidApi, HidDevice};

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
                .map_err(|e| HidTransportError::Backend(e.to_string()))?;
            Ok(())
        }

        fn get_feature_report(
            &self,
        ) -> Result<[u8; FEATURE_REPORT_SIZE], HidTransportError> {
            let mut buffer = [0u8; FEATURE_REPORT_SIZE + 1];
            self.device
                .get_feature_report(&mut buffer)
                .map_err(|e| HidTransportError::Backend(e.to_string()))?;

            let mut report = [0u8; FEATURE_REPORT_SIZE];
            report.copy_from_slice(&buffer[1..]);
            Ok(report)
        }
    }

    pub fn open_feature_device(
        vendor_id: u16,
        product_id: u16,
        usage_page: u16,
    ) -> Result<Box<dyn FeatureTransport>, HidTransportError> {
        let api = HidApi::new().map_err(|e| HidTransportError::Backend(e.to_string()))?;

        let info = api
            .device_list()
            .find(|device| {
                device.vendor_id() == vendor_id
                    && device.product_id() == product_id
                    && device.usage_page() == usage_page
            })
            .ok_or(HidTransportError::NotFound)?;

        let device = info
            .open_device(&api)
            .map_err(|e| HidTransportError::Backend(e.to_string()))?;

        Ok(Box::new(WindowsFeatureTransport { device }))
    }
}

#[cfg(windows)]
pub use windows::open_feature_device;

#[cfg(not(windows))]
pub fn open_feature_device(
    _vendor_id: u16,
    _product_id: u16,
    _usage_page: u16,
) -> Result<Box<dyn FeatureTransport>, HidTransportError> {
    Err(HidTransportError::UnsupportedPlatform)
}
