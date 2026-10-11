use super::{PersistenceError, PersistenceStore, unix_millis};
use mechkeymesh_config::{ConfigurationScope, DesiredPerKeyRgb, KeyColor};
use mechkeymesh_driver_api::{Capability, RgbColor};
use rusqlite::{OptionalExtension, TransactionBehavior, params};
use std::collections::HashSet;

const BRIGHTNESS_SETTING_KEY: &str = "brightness";
const DEFAULT_BRIGHTNESS: u8 = 5;

impl PersistenceStore {
    pub fn save_per_key_rgb(&self, desired: &DesiredPerKeyRgb) -> Result<(), PersistenceError> {
        validate_scope(&desired.scope)?;
        validate_desired(desired)?;

        let mut connection = self.connection.lock().expect("database lock poisoned");
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

        let profile_exists: bool = transaction.query_row(
            r#"
                SELECT EXISTS(
                    SELECT 1
                    FROM profiles
                    WHERE device_model_id = ?1
                      AND id = ?2
                )
            "#,
            params![desired.scope.device_model_id, desired.scope.profile_id],
            |row| row.get(0),
        )?;

        if !profile_exists {
            return Err(PersistenceError::ProfileNotFound {
                device_model_id: desired.scope.device_model_id.clone(),
                profile_id: desired.scope.profile_id.clone(),
            });
        }

        let capability = desired.scope.capability.storage_key();
        transaction.execute(
            r#"
                DELETE FROM per_key_lighting
                WHERE device_model_id = ?1
                  AND profile_id = ?2
                  AND capability = ?3
            "#,
            params![
                desired.scope.device_model_id,
                desired.scope.profile_id,
                capability
            ],
        )?;

        let now = unix_millis();
        for key in &desired.colors {
            if key.color.is_black() {
                continue;
            }

            transaction.execute(
                r#"
                    INSERT INTO per_key_lighting (
                        device_model_id,
                        profile_id,
                        capability,
                        key_id,
                        r,
                        g,
                        b,
                        updated_at
                    )
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                "#,
                params![
                    desired.scope.device_model_id,
                    desired.scope.profile_id,
                    capability,
                    key.key_id,
                    i64::from(key.color.r),
                    i64::from(key.color.g),
                    i64::from(key.color.b),
                    now
                ],
            )?;
        }

        transaction.execute(
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
                ON CONFLICT(
                    device_model_id,
                    profile_id,
                    capability,
                    setting_key
                ) DO UPDATE SET
                    value_json = excluded.value_json,
                    updated_at = excluded.updated_at
            "#,
            params![
                desired.scope.device_model_id,
                desired.scope.profile_id,
                capability,
                BRIGHTNESS_SETTING_KEY,
                desired.brightness.to_string(),
                now
            ],
        )?;

        transaction.commit()?;
        Ok(())
    }

    pub fn load_per_key_rgb(
        &self,
        scope: &ConfigurationScope,
    ) -> Result<DesiredPerKeyRgb, PersistenceError> {
        validate_scope(scope)?;
        let connection = self.connection.lock().expect("database lock poisoned");
        let capability = scope.capability.storage_key();

        let brightness = connection
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
                    capability,
                    BRIGHTNESS_SETTING_KEY
                ],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(|raw| {
                raw.parse::<u8>().map_err(|error| {
                    PersistenceError::Integrity(format!(
                        "invalid per-key RGB brightness value {raw:?}: {error}"
                    ))
                })
            })
            .transpose()?
            .unwrap_or(DEFAULT_BRIGHTNESS);

        let mut statement = connection.prepare(
            r#"
                SELECT key_id, r, g, b
                FROM per_key_lighting
                WHERE device_model_id = ?1
                  AND profile_id = ?2
                  AND capability = ?3
                ORDER BY key_id
            "#,
        )?;

        let rows = statement.query_map(
            params![scope.device_model_id, scope.profile_id, capability],
            |row| {
                Ok(KeyColor {
                    key_id: row.get(0)?,
                    color: RgbColor {
                        r: row.get(1)?,
                        g: row.get(2)?,
                        b: row.get(3)?,
                    },
                })
            },
        )?;

        let colors = rows.collect::<Result<Vec<_>, _>>()?;
        let desired = DesiredPerKeyRgb::new(scope.clone(), brightness, colors);
        validate_desired(&desired)?;
        Ok(desired)
    }
}

fn validate_scope(scope: &ConfigurationScope) -> Result<(), PersistenceError> {
    if scope.capability != Capability::PerKeyRgb {
        return Err(PersistenceError::InvalidScope(format!(
            "per-key RGB requires capability {}, got {}",
            Capability::PerKeyRgb.storage_key(),
            scope.capability.storage_key()
        )));
    }
    if scope.device_model_id.trim().is_empty() || scope.profile_id.trim().is_empty() {
        return Err(PersistenceError::InvalidScope(
            "device model and profile IDs must not be empty".into(),
        ));
    }
    Ok(())
}

fn validate_desired(desired: &DesiredPerKeyRgb) -> Result<(), PersistenceError> {
    if desired.brightness > 5 {
        return Err(PersistenceError::InvalidScope(
            "per-key RGB brightness must be between 0 and 5".into(),
        ));
    }

    let mut seen = HashSet::new();
    for key in &desired.colors {
        if key.key_id.trim().is_empty() {
            return Err(PersistenceError::InvalidScope(
                "per-key RGB key ID must not be empty".into(),
            ));
        }
        if !seen.insert(key.key_id.as_str()) {
            return Err(PersistenceError::InvalidScope(format!(
                "duplicate per-key RGB key: {}",
                key.key_id
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mechkeymesh_config::DeviceModel;
    use tempfile::tempdir;

    fn setup() -> (tempfile::TempDir, std::path::PathBuf) {
        let directory = tempdir().unwrap();
        let path = directory.path().join("mechkeymesh.sqlite3");
        let store = PersistenceStore::open(&path).unwrap();
        store
            .ensure_device_model(&DeviceModel::new("aula.f108pro", "AULA", "AULA F108 Pro"))
            .unwrap();
        store.ensure_default_profile("aula.f108pro").unwrap();
        drop(store);
        (directory, path)
    }

    fn scope() -> ConfigurationScope {
        ConfigurationScope::new("aula.f108pro", "default", Capability::PerKeyRgb)
    }

    #[test]
    fn per_key_rgb_survives_database_reopen() {
        let (_directory, path) = setup();
        let desired = DesiredPerKeyRgb::new(
            scope(),
            4,
            vec![
                KeyColor::new("key-001", RgbColor { r: 255, g: 0, b: 0 }),
                KeyColor::new("key-056", RgbColor { r: 0, g: 255, b: 0 }),
            ],
        );

        PersistenceStore::open(&path)
            .unwrap()
            .save_per_key_rgb(&desired)
            .unwrap();

        let reopened = PersistenceStore::open(&path).unwrap();
        assert_eq!(reopened.load_per_key_rgb(&scope()).unwrap(), desired);
    }

    #[test]
    fn replacement_clears_old_colors_and_black_is_not_stored() {
        let (_directory, path) = setup();
        let store = PersistenceStore::open(&path).unwrap();
        store
            .save_per_key_rgb(&DesiredPerKeyRgb::new(
                scope(),
                5,
                vec![
                    KeyColor::new("key-001", RgbColor { r: 255, g: 0, b: 0 }),
                    KeyColor::new("key-002", RgbColor { r: 0, g: 0, b: 255 }),
                ],
            ))
            .unwrap();

        let replacement = DesiredPerKeyRgb::new(
            scope(),
            2,
            vec![
                KeyColor::new("key-002", RgbColor::BLACK),
                KeyColor::new("key-056", RgbColor { r: 0, g: 255, b: 0 }),
            ],
        );
        store.save_per_key_rgb(&replacement).unwrap();

        let loaded = store.load_per_key_rgb(&scope()).unwrap();
        assert_eq!(loaded.brightness, 2);
        assert_eq!(
            loaded.colors,
            vec![KeyColor::new(
                "key-056",
                RgbColor { r: 0, g: 255, b: 0 }
            )]
        );
    }

    #[test]
    fn rejects_wrong_scope_duplicate_keys_and_invalid_brightness() {
        let (_directory, path) = setup();
        let store = PersistenceStore::open(&path).unwrap();

        let wrong = DesiredPerKeyRgb::new(
            ConfigurationScope::new("aula.f108pro", "default", Capability::HardwareRgb),
            5,
            vec![],
        );
        assert!(store.save_per_key_rgb(&wrong).is_err());

        let duplicate = DesiredPerKeyRgb::new(
            scope(),
            5,
            vec![
                KeyColor::new("key-001", RgbColor { r: 1, g: 2, b: 3 }),
                KeyColor::new("key-001", RgbColor { r: 4, g: 5, b: 6 }),
            ],
        );
        assert!(store.save_per_key_rgb(&duplicate).is_err());

        let too_bright = DesiredPerKeyRgb::new(scope(), 6, vec![]);
        assert!(store.save_per_key_rgb(&too_bright).is_err());
    }

    #[test]
    fn missing_profile_is_rejected() {
        let (_directory, path) = setup();
        let store = PersistenceStore::open(&path).unwrap();
        let desired = DesiredPerKeyRgb::new(
            ConfigurationScope::new("aula.f108pro", "missing", Capability::PerKeyRgb),
            5,
            vec![],
        );
        assert!(matches!(
            store.save_per_key_rgb(&desired).unwrap_err(),
            PersistenceError::ProfileNotFound { .. }
        ));
    }
}
