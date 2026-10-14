use super::AulaF108ProSession;
use mechkeymesh_driver_api::{
    DeviceSettings, DeviceSettingsDescriptor, DriverError, FnActivationMode,
    FnActivationModeOption, KeyResponseLevelOption, OperationContext, SleepTimeout,
    SleepTimeoutOption,
};
use mechkeymesh_hid::FEATURE_REPORT_SIZE;

const SETTINGS_COMMAND: u8 = 0x17;
const SETTINGS_TAG: [u8; 2] = [0x00, 0x01];
const ALT_TAB_OFFSET: usize = 2;
const ALT_F4_OFFSET: usize = 3;
const WINDOWS_KEY_OFFSET: usize = 4;
const FN_MODE_OFFSET: usize = 5;
const SLEEP_OFFSET: usize = 6;
const RESPONSE_OFFSET: usize = 8;

pub fn descriptor() -> DeviceSettingsDescriptor {
    DeviceSettingsDescriptor {
        sleep_options: vec![
            sleep_option(SleepTimeout::Never, 0, "Never"),
            sleep_option(SleepTimeout::OneMinute, 1, "1 minute"),
            sleep_option(SleepTimeout::FiveMinutes, 2, "5 minutes"),
            sleep_option(SleepTimeout::ThirtyMinutes, 3, "30 minutes"),
        ],
        fn_modes: vec![
            FnActivationModeOption {
                mode: FnActivationMode::Toggle,
                raw_value: 0,
            },
            FnActivationModeOption {
                mode: FnActivationMode::Momentary,
                raw_value: 1,
            },
        ],
        key_response_levels: vec![
            response_option(1, "Fastest", "2–3 ms", "5–6 ms", "12–13 ms"),
            response_option(2, "Fast", "5–6 ms", "7–9 ms", "15–16 ms"),
            response_option(3, "Balanced", "8–9 ms", "10–12 ms", "18–19 ms"),
            response_option(4, "Stable", "13–14 ms", "15–17 ms", "23–24 ms"),
            response_option(
                5,
                "Maximum debounce",
                "17–18 ms",
                "19–21 ms",
                "27–28 ms",
            ),
        ],
        hardware_write_verified: false,
        note: "The F108 field offsets come from its OEM binary. Sleep raw 0–3 and response raw 1–5 are independently confirmed by captures of the same 0C45:800A 04 17 settings protocol; this exact F108 write still requires change/verify/restore hardware testing.".into(),
    }
}

pub fn validate(settings: &DeviceSettings) -> Result<(), DriverError> {
    if !(1..=5).contains(&settings.key_response_level) {
        return Err(DriverError::InvalidConfiguration(format!(
            "F108 key response level must be 1 through 5, got {}",
            settings.key_response_level
        )));
    }
    Ok(())
}

pub fn build_data(settings: &DeviceSettings) -> Result<[u8; FEATURE_REPORT_SIZE], DriverError> {
    validate(settings)?;
    let mut data = [0; FEATURE_REPORT_SIZE];
    data[..2].copy_from_slice(&SETTINGS_TAG);
    data[ALT_TAB_OFFSET] = u8::from(settings.disable_alt_tab);
    data[ALT_F4_OFFSET] = u8::from(settings.disable_alt_f4);
    data[WINDOWS_KEY_OFFSET] = u8::from(settings.disable_windows_key);
    data[FN_MODE_OFFSET] = match settings.fn_mode {
        FnActivationMode::Toggle => 0,
        FnActivationMode::Momentary => 1,
    };
    data[SLEEP_OFFSET] = match settings.sleep_timeout {
        SleepTimeout::Never => 0,
        SleepTimeout::OneMinute => 1,
        SleepTimeout::FiveMinutes => 2,
        SleepTimeout::ThirtyMinutes => 3,
    };
    data[RESPONSE_OFFSET] = settings.key_response_level;
    data[FEATURE_REPORT_SIZE - 2] = 0xAA;
    data[FEATURE_REPORT_SIZE - 1] = 0x55;
    Ok(data)
}

pub fn build_wireless_data(settings: &DeviceSettings) -> Result<[u8; 16], DriverError> {
    validate(settings)?;
    let mut data = [0u8; 16];
    data[..2].copy_from_slice(&SETTINGS_TAG);
    data[ALT_TAB_OFFSET] = u8::from(settings.disable_alt_tab);
    data[ALT_F4_OFFSET] = u8::from(settings.disable_alt_f4);
    data[WINDOWS_KEY_OFFSET] = u8::from(settings.disable_windows_key);
    data[FN_MODE_OFFSET] = match settings.fn_mode {
        FnActivationMode::Toggle => 0,
        FnActivationMode::Momentary => 1,
    };
    data[SLEEP_OFFSET] = match settings.sleep_timeout {
        SleepTimeout::Never => 0,
        SleepTimeout::OneMinute => 1,
        SleepTimeout::FiveMinutes => 2,
        SleepTimeout::ThirtyMinutes => 3,
    };
    data[RESPONSE_OFFSET] = settings.key_response_level;
    data[14] = 0xAA;
    data[15] = 0x55;
    Ok(data)
}

pub fn send(
    session: &mut AulaF108ProSession,
    context: &OperationContext,
    settings: &DeviceSettings,
) -> Result<(), DriverError> {
    // Build and validate the complete block before beginning the transaction.
    let data = build_data(settings)?;
    session.begin_transaction(context)?;
    let mut init = [0; FEATURE_REPORT_SIZE];
    init[0] = 0x04;
    init[1] = SETTINGS_COMMAND;
    init[2] = 0x01;
    init[8] = 0x01;
    session.send(context, init, true)?;
    session.send(context, data, false)?;
    session.apply_transaction(context)
}

fn sleep_option(timeout: SleepTimeout, raw_value: u8, label: &str) -> SleepTimeoutOption {
    SleepTimeoutOption {
        timeout,
        raw_value,
        label: label.into(),
    }
}

fn response_option(
    level: u8,
    label: &str,
    wired_timing: &str,
    wireless_timing: &str,
    bluetooth_timing: &str,
) -> KeyResponseLevelOption {
    KeyResponseLevelOption {
        level,
        raw_value: level,
        label: label.into(),
        wired_timing: wired_timing.into(),
        wireless_timing: wireless_timing.into(),
        bluetooth_timing: bluetooth_timing.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> DeviceSettings {
        DeviceSettings {
            disable_windows_key: true,
            disable_alt_tab: true,
            disable_alt_f4: false,
            fn_mode: FnActivationMode::Momentary,
            sleep_timeout: SleepTimeout::FiveMinutes,
            key_response_level: 3,
        }
    }

    #[test]
    fn encodes_complete_oem_settings_block() {
        let data = build_data(&sample()).unwrap();
        assert_eq!(&data[..9], &[0, 1, 1, 0, 1, 1, 2, 0, 3]);
        assert_eq!(&data[62..64], &[0xAA, 0x55]);
    }

    #[test]
    fn encodes_wireless_settings_block_at_the_same_field_offsets() {
        let data = build_wireless_data(&sample()).unwrap();
        assert_eq!(&data[..9], &[0, 1, 1, 0, 1, 1, 2, 0, 3]);
        assert_eq!(&data[14..16], &[0xAA, 0x55]);
    }

    #[test]
    fn exposes_confirmed_raw_enums_and_timings() {
        let descriptor = descriptor();
        assert_eq!(descriptor.sleep_options[0].raw_value, 0);
        assert_eq!(descriptor.sleep_options[3].raw_value, 3);
        assert_eq!(descriptor.fn_modes[0].raw_value, 0);
        assert_eq!(descriptor.fn_modes[1].raw_value, 1);
        assert_eq!(descriptor.key_response_levels[0].raw_value, 1);
        assert_eq!(descriptor.key_response_levels[4].raw_value, 5);
        assert!(!descriptor.hardware_write_verified);
    }

    #[test]
    fn rejects_response_values_outside_one_through_five() {
        let mut settings = sample();
        settings.key_response_level = 0;
        assert!(build_data(&settings).is_err());
        settings.key_response_level = 6;
        assert!(build_data(&settings).is_err());
    }
}
