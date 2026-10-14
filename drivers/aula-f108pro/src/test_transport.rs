use super::*;
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub(crate) struct Reports {
    pub writes: Vec<[u8; FEATURE_REPORT_SIZE]>,
    pub reads: Vec<[u8; FEATURE_REPORT_SIZE]>,
    pub replies: Vec<[u8; FEATURE_REPORT_SIZE]>,
}

struct EchoTransport {
    reports: Arc<Mutex<Reports>>,
    live: bool,
}

impl FeatureTransport for EchoTransport {
    fn set_feature_report(
        &self,
        report: &[u8; FEATURE_REPORT_SIZE],
    ) -> Result<(), HidTransportError> {
        self.reports.lock().unwrap().writes.push(*report);
        Ok(())
    }

    fn get_feature_report(&self) -> Result<[u8; FEATURE_REPORT_SIZE], HidTransportError> {
        let mut reports = self.reports.lock().unwrap();
        let sent = *reports.writes.last().unwrap();
        reports.reads.push(sent);
        if self.live {
            reports.replies.push(sent);
            return Ok(sent);
        }
        if sent[..2] == [4, 0xf0] {
            return Err(HidTransportError::Timeout("flash has no ACK".into()));
        }
        if sent[0] == 4 {
            let mut reply = sent;
            reply[3] = 1;
            reports.replies.push(reply);
            Ok(reply)
        } else {
            let start = reports
                .writes
                .iter()
                .rposition(|r| r[..2] == [4, 0x23])
                .unwrap();
            let reply = reports.writes[start + 1];
            reports.replies.push(reply);
            Ok(reply)
        }
    }
}

pub(crate) fn session(live: bool) -> (AulaF108ProSession, Arc<Mutex<Reports>>) {
    let reports = Arc::new(Mutex::new(Reports::default()));
    (
        AulaF108ProSession {
            candidate: DeviceCandidate {
                driver_id: DRIVER_ID.into(),
                instance_id: "rgb-test".into(),
                name: "AULA F108 Pro".into(),
                connection: Connection::Usb,
                vendor_id: Some(VENDOR_ID),
                product_id: Some(PRODUCT_ID),
                usage_page: Some(CONFIG_USAGE_PAGE),
                interface_number: Some(3),
                serial_number: None,
            },
            transport: Box::new(EchoTransport {
                reports: Arc::clone(&reports),
                live,
            }),
            lcd_transport: None,
        },
        reports,
    )
}
