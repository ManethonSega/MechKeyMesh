use super::{PersistenceError, PersistenceStore, unix_millis};
use mechkeymesh_config::ConfigurationScope;
use mechkeymesh_driver_api::{Capability, DeviceSettings};
use rusqlite::{OptionalExtension, params};

const SETTINGS_KEY: &str = "complete-settings-block";

impl PersistenceStore {
    pub fn save_device_settings(
        &self,
        scope: &ConfigurationScope,
        settings: &DeviceSettings,
    ) -> Result<(), PersistenceError> {
        validate_scope(scope)?;
        let value_json = serde_json::to_string(settings).map_err(|error| {
            PersistenceError::Integrity(format!("could not serialize device settings: {error}"))
        })?;
        let connection = self.connection.lock().expect("database lock poisoned");
        connection.execute(
            r#"
                INSERT INTO device_settings (
                    device_model_id,
                    profile_id,
                    capability,
                    setting_key,
                    value_json,
                    updated_at
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                ON CONFLICT(device_model_id, profile_id, capability, setting_key)
                DO UPDATE SET
                    value_json = excluded.value_json,
                    updated_at = excluded.updated_at
            "#,
            params![
                scope.device_model_id,
                scope.profile_id,
                scope.capability.storage_key(),
                SETTINGS_KEY,
                value_json,
                unix_millis()
            ],
        )?;
        Ok(())
    }

    pub fn load_device_settings(
        &self,
        scope: &ConfigurationScope,
    ) -> Result<Option<DeviceSettings>, PersistenceError> {
        validate_scope(scope)?;
        let connection = self.connection.lock().expect("database lock poisoned");
        let value_json = connection
            .query_row(
                r#"
                    SELECT value_json
                    FROM device_settings
                    WHERE device_model_id = ?1
                      AND profile_id = ?2
                      AND capability = ?3
                      AND setting_key = ?4
                "#,
                params![
                    scope.device_model_id,
                    scope.profile_id,
                    scope.capability.storage_key(),
                    SETTINGS_KEY
                ],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        value_json
            .map(|json| {
                serde_json::from_str(&json).map_err(|error| {
                    PersistenceError::Integrity(format!(
                        "invalid persisted device settings: {error}"
                    ))
                })
            })
            .transpose()
    }
}

fn validate_scope(scope: &ConfigurationScope) -> Result<(), PersistenceError> {
    if scope.capability != Capability::DeviceSettings {
        return Err(PersistenceError::InvalidScope(format!(
            "device settings require capability {}, got {}",
            Capability::DeviceSettings.storage_key(),
            scope.capability.storage_key()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mechkeymesh_config::DeviceModel;
    use mechkeymesh_driver_api::{FnActivationMode, SleepTimeout};
    use tempfile::tempdir;

    #[test]
    fn complete_settings_block_survives_reopen() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("settings.sqlite3");
        let expected = DeviceSettings {
            disable_windows_key: true,
            disable_alt_tab: false,
            disable_alt_f4: true,
            fn_mode: FnActivationMode::Momentary,
            sleep_timeout: SleepTimeout::FiveMinutes,
            key_response_level: 3,
        };
        let scope = ConfigurationScope::new("aula.f108pro", "default", Capability::DeviceSettings);
        {
            let store = PersistenceStore::open(&path).unwrap();
            store
                .ensure_device_model(&DeviceModel::new("aula.f108pro", "AULA", "AULA F108 Pro"))
                .unwrap();
            store.ensure_default_profile("aula.f108pro").unwrap();
            store.save_device_settings(&scope, &expected).unwrap();
        }
        let reopened = PersistenceStore::open(&path).unwrap();
        assert_eq!(
            reopened.load_device_settings(&scope).unwrap(),
            Some(expected)
        );
    }
}
