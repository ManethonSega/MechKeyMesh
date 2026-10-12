use crate::{
    build_lighting_payload, map_transport_error, per_key_rgb, remap, settings,
};
use mechkeymesh_driver_api::{
    Capability, Connection, DeviceCandidate, DeviceDescriptor, DeviceSettings, DriverError,
    KeyAssignment, KeymapLayer, LightingConfig, OperationContext, PerKeyRgbConfig,
};
use mechkeymesh_hid::{INTERRUPT_REPORT_SIZE, InterruptTransport};
use std::time::{Duration, Instant};

pub const WIRELESS_VENDOR_ID: u16 = 0x05AC;
pub const WIRELESS_PRODUCT_ID: u16 = 0x024F;
pub const WIRELESS_USAGE_PAGE: u16 = 0xFF60;
pub const WIRELESS_INTERFACE_NUMBER: i32 = 3;

const PROBE_COMMAND: u8 = 0x02;
const LIGHTING_COMMAND: u8 = 0x05;
const SETTINGS_COMMAND: u8 = 0x07;
const NORMAL_KEYMAP_COMMAND: u8 = 0x10;
const FN_KEYMAP_COMMAND: u8 = 0x12;
const PER_KEY_RGB_COMMAND: u8 = 0x14;
const BATTERY_COMMAND: u8 = 0x20;
const DATA_BYTES_PER_PACKET: usize = 28;
const FIRST_REPLY_DELAY: Duration = Duration::from_millis(5);
const REPLY_TIMEOUT: Duration = Duration::from_millis(5);
const REPLY_RETRY_DELAY: Duration = Duration::from_millis(10);
const CHUNK_DELAY: Duration = Duration::from_millis(2);
const BATTERY_INTERVAL: Duration = Duration::from_secs(15);
const REPLY_ATTEMPTS: usize = 20;

pub const fn capabilities() -> &'static [Capability] {
    &[
        Capability::Keys,
        Capability::FnLayer,
        Capability::HardwareRgb,
        Capability::PerKeyRgb,
        Capability::DeviceSettings,
        Capability::Battery,
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WirelessPacket([u8; INTERRUPT_REPORT_SIZE]);

impl WirelessPacket {
    pub fn new(command: u8, subcommand: u8, index: u8, data: &[u8]) -> Result<Self, DriverError> {
        if data.len() > DATA_BYTES_PER_PACKET {
            return Err(DriverError::InvalidConfiguration(format!(
                "F108 2.4 GHz packet has {} data bytes; maximum is {DATA_BYTES_PER_PACKET}",
                data.len()
            )));
        }
        let mut report = [0u8; INTERRUPT_REPORT_SIZE];
        report[0] = command;
        report[1] = subcommand;
        report[2] = index;
        report[3..3 + data.len()].copy_from_slice(data);
        report[INTERRUPT_REPORT_SIZE - 1] = report[..INTERRUPT_REPORT_SIZE - 1]
            .iter()
            .fold(0u8, |sum, byte| sum.wrapping_add(*byte));
        Ok(Self(report))
    }

    pub const fn bytes(&self) -> &[u8; INTERRUPT_REPORT_SIZE] {
        &self.0
    }

    fn echoed_by(&self, response: &[u8; INTERRUPT_REPORT_SIZE]) -> bool {
        response[..3] == self.0[..3]
    }
}

pub struct AulaF108WirelessSession {
    candidate: DeviceCandidate,
    transport: Box<dyn InterruptTransport>,
    battery_percent: Option<u8>,
    last_battery_poll: Option<Instant>,
}

impl AulaF108WirelessSession {
    pub fn new(candidate: DeviceCandidate, transport: Box<dyn InterruptTransport>) -> Self {
        Self {
            candidate,
            transport,
            battery_percent: None,
            last_battery_poll: None,
        }
    }

    pub fn descriptor(&self) -> DeviceDescriptor {
        DeviceDescriptor {
            driver_id: self.candidate.driver_id.clone(),
            device_model_id: "aula.f108pro".into(),
            instance_id: self.candidate.instance_id.clone(),
            vendor: "AULA".into(),
            name: "AULA F108 Pro 2.4G".into(),
            connection: Connection::Wireless24G,
            vendor_id: self.candidate.vendor_id,
            product_id: self.candidate.product_id,
            capabilities: capabilities().to_vec(),
            firmware: None,
            battery_percent: self.battery_percent,
        }
    }

    pub fn initialize(&mut self, context: &OperationContext) -> Result<(), DriverError> {
        self.probe(context)?;
        self.refresh_battery(context)
    }

    pub fn health_check(&mut self, context: &OperationContext) -> Result<(), DriverError> {
        self.probe(context)?;
        if self
            .last_battery_poll
            .is_none_or(|last| last.elapsed() >= BATTERY_INTERVAL)
        {
            self.refresh_battery(context)?;
        }
        Ok(())
    }

    pub fn set_lighting(
        &self,
        context: &OperationContext,
        config: LightingConfig,
    ) -> Result<(), DriverError> {
        let data = build_lighting_payload(config)?;
        self.exchange(
            context,
            WirelessPacket::new(LIGHTING_COMMAND, 0x10, 0, &data[..16])?,
        )?;
        Ok(())
    }

    pub fn set_device_settings(
        &self,
        context: &OperationContext,
        device_settings: &DeviceSettings,
    ) -> Result<(), DriverError> {
        let data = settings::build_wireless_data(device_settings)?;
        self.exchange(
            context,
            WirelessPacket::new(SETTINGS_COMMAND, 0x10, 0, &data)?,
        )?;
        Ok(())
    }

    pub fn set_keymap(
        &self,
        context: &OperationContext,
        layer: KeymapLayer,
        assignments: &[KeyAssignment],
    ) -> Result<(), DriverError> {
        let table = remap::build_remap_table(layer, assignments)?;
        let command = match layer {
            KeymapLayer::Normal => NORMAL_KEYMAP_COMMAND,
            KeymapLayer::Fn => FN_KEYMAP_COMMAND,
        };
        self.send_chunks(context, command, &table)
    }

    pub fn set_per_key_rgb(
        &self,
        context: &OperationContext,
        config: &PerKeyRgbConfig,
    ) -> Result<(), DriverError> {
        let header = per_key_rgb::brightness_preamble_payload(config.brightness)?;
        self.exchange(
            context,
            WirelessPacket::new(LIGHTING_COMMAND, 0x10, 0, &header[..16])?,
        )?;
        let table = per_key_rgb::build_rgb_table(config)?;
        self.send_chunks(context, PER_KEY_RGB_COMMAND, &table)
    }

    pub(crate) fn probe(&self, context: &OperationContext) -> Result<(), DriverError> {
        self.exchange(context, WirelessPacket::new(PROBE_COMMAND, 0, 0, &[])?)?;
        Ok(())
    }

    fn refresh_battery(&mut self, context: &OperationContext) -> Result<(), DriverError> {
        let response = self.exchange(
            context,
            WirelessPacket::new(BATTERY_COMMAND, 0x01, 0, &[])?,
        )?;
        self.battery_percent = Some(response[3].min(100));
        self.last_battery_poll = Some(Instant::now());
        Ok(())
    }

    fn send_chunks(
        &self,
        context: &OperationContext,
        command: u8,
        data: &[u8],
    ) -> Result<(), DriverError> {
        for (index, chunk) in data.chunks(DATA_BYTES_PER_PACKET).enumerate() {
            let index = u8::try_from(index).map_err(|_| {
                DriverError::InvalidConfiguration(
                    "F108 2.4 GHz transfer needs more than 256 chunks".into(),
                )
            })?;
            self.exchange(
                context,
                WirelessPacket::new(command, chunk.len() as u8, index, chunk)?,
            )?;
            context.sleep(CHUNK_DELAY)?;
        }
        Ok(())
    }

    fn exchange(
        &self,
        context: &OperationContext,
        packet: WirelessPacket,
    ) -> Result<[u8; INTERRUPT_REPORT_SIZE], DriverError> {
        context.check()?;
        self.transport
            .write_output_report(packet.bytes())
            .map_err(map_transport_error)?;
        context.sleep(FIRST_REPLY_DELAY)?;

        for attempt in 0..REPLY_ATTEMPTS {
            context.check()?;
            if let Some(response) = self
                .transport
                .read_input_report(REPLY_TIMEOUT)
                .map_err(map_transport_error)?
                && packet.echoed_by(&response)
            {
                return Ok(response);
            }
            if attempt + 1 < REPLY_ATTEMPTS {
                context.sleep(REPLY_RETRY_DELAY)?;
            }
        }

        Err(DriverError::Timeout(
            "F108 2.4 GHz dongle did not echo the command; the keyboard may be asleep or switched to another connection"
                .into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seals_packet_with_wrapping_checksum() {
        let packet = WirelessPacket::new(0x05, 0x10, 0, &[1, 255, 0, 0]).unwrap();
        let expected = packet.bytes()[..31]
            .iter()
            .fold(0u8, |sum, byte| sum.wrapping_add(*byte));
        assert_eq!(packet.bytes()[31], expected);
        assert_eq!(&packet.bytes()[..7], &[0x05, 0x10, 0, 1, 255, 0, 0]);
    }

    #[test]
    fn rejects_more_than_twenty_eight_data_bytes() {
        assert!(WirelessPacket::new(1, 2, 3, &[0; 29]).is_err());
    }

    #[test]
    fn wireless_capability_matrix_excludes_unimplemented_or_unsupported_features() {
        let caps = capabilities();
        assert!(caps.contains(&Capability::HardwareRgb));
        assert!(caps.contains(&Capability::PerKeyRgb));
        assert!(caps.contains(&Capability::Battery));
        assert!(!caps.contains(&Capability::Display));
        assert!(!caps.contains(&Capability::LiveRgb));
        assert!(!caps.contains(&Capability::Music));
        assert!(!caps.contains(&Capability::Macros));
    }
}
