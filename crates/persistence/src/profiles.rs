use super::{PersistenceError, PersistenceStore, unix_millis};
use mechkeymesh_config::Profile;
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationRule {
    pub id: String,
    pub device_model_id: String,
    pub profile_id: String,
    pub executable_pattern: String,
    pub priority: i32,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProfileHostState {
    pub live_scene_json: Option<String>,
    pub music_config_json: Option<String>,
    pub host_mode: String,
    pub lcd_asset_path: Option<String>,
    pub lcd_content_hash: Option<String>,
}

impl PersistenceStore {
    pub fn list_profiles(&self, device_model_id: &str) -> Result<Vec<Profile>, PersistenceError> {
        let connection = self.connection.lock().expect("database lock poisoned");
        let mut statement = connection.prepare(
            "SELECT id, name, is_default FROM profiles WHERE device_model_id = ?1 ORDER BY is_default DESC, name COLLATE NOCASE, id",
        )?;
        let rows = statement.query_map([device_model_id], |row| {
            Ok(Profile {
                id: row.get(0)?,
                device_model_id: device_model_id.to_string(),
                name: row.get(1)?,
                is_default: row.get(2)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(PersistenceError::from)
    }

    pub fn clone_profile(
        &self,
        device_model_id: &str,
        source_profile_id: &str,
        id: &str,
        name: &str,
    ) -> Result<Profile, PersistenceError> {
        validate_identity(id, name)?;
        let mut connection = self.connection.lock().expect("database lock poisoned");
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        ensure_profile(&transaction, device_model_id, source_profile_id)?;
        let now = unix_millis();
        transaction.execute(
            "INSERT INTO profiles (device_model_id, id, name, is_default, created_at, updated_at) VALUES (?1, ?2, ?3, 0, ?4, ?4)",
            params![device_model_id, id, name.trim(), now],
        )?;
        clone_rows(&transaction, device_model_id, source_profile_id, id)?;
        transaction.commit()?;
        Ok(Profile {
            id: id.into(),
            device_model_id: device_model_id.into(),
            name: name.trim().into(),
            is_default: false,
        })
    }

    pub fn rename_profile(
        &self,
        device_model_id: &str,
        profile_id: &str,
        name: &str,
    ) -> Result<(), PersistenceError> {
        validate_identity(profile_id, name)?;
        let connection = self.connection.lock().expect("database lock poisoned");
        let changed = connection.execute(
            "UPDATE profiles SET name = ?3, updated_at = ?4 WHERE device_model_id = ?1 AND id = ?2",
            params![device_model_id, profile_id, name.trim(), unix_millis()],
        )?;
        if changed == 0 {
            return Err(PersistenceError::ProfileNotFound {
                device_model_id: device_model_id.into(),
                profile_id: profile_id.into(),
            });
        }
        Ok(())
    }

    pub fn delete_profile(
        &self,
        device_model_id: &str,
        profile_id: &str,
    ) -> Result<(), PersistenceError> {
        let connection = self.connection.lock().expect("database lock poisoned");
        let changed = connection.execute(
            "DELETE FROM profiles WHERE device_model_id = ?1 AND id = ?2 AND is_default = 0",
            params![device_model_id, profile_id],
        )?;
        if changed == 0 {
            return Err(PersistenceError::InvalidScope(
                "default or missing profile cannot be deleted".into(),
            ));
        }
        Ok(())
    }

    pub fn list_application_rules(
        &self,
        device_model_id: &str,
    ) -> Result<Vec<ApplicationRule>, PersistenceError> {
        let connection = self.connection.lock().expect("database lock poisoned");
        let mut statement = connection.prepare(
            r#"
                SELECT id, profile_id, executable_pattern, priority, enabled
                FROM application_rules
                WHERE device_model_id = ?1 AND capability = 'profile'
                ORDER BY priority DESC, id
            "#,
        )?;
        let rows = statement.query_map([device_model_id], |row| {
            Ok(ApplicationRule {
                id: row.get(0)?,
                device_model_id: device_model_id.into(),
                profile_id: row.get(1)?,
                executable_pattern: row.get(2)?,
                priority: row.get(3)?,
                enabled: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(PersistenceError::from)
    }

    pub fn save_application_rule(&self, rule: &ApplicationRule) -> Result<(), PersistenceError> {
        if rule.id.trim().is_empty() || rule.executable_pattern.trim().is_empty() {
            return Err(PersistenceError::InvalidScope(
                "application rule ID and executable pattern must not be empty".into(),
            ));
        }
        let connection = self.connection.lock().expect("database lock poisoned");
        let now = unix_millis();
        connection.execute(
            r#"
                INSERT INTO application_rules (
                    device_model_id, profile_id, capability, id,
                    executable_pattern, priority, enabled, created_at, updated_at
                ) VALUES (?1, ?2, 'profile', ?3, ?4, ?5, ?6, ?7, ?7)
                ON CONFLICT(device_model_id, profile_id, capability, id) DO UPDATE SET
                    executable_pattern = excluded.executable_pattern,
                    priority = excluded.priority,
                    enabled = excluded.enabled,
                    updated_at = excluded.updated_at
            "#,
            params![
                rule.device_model_id,
                rule.profile_id,
                rule.id,
                rule.executable_pattern.trim(),
                rule.priority,
                rule.enabled,
                now
            ],
        )?;
        Ok(())
    }

    pub fn delete_application_rule(
        &self,
        device_model_id: &str,
        profile_id: &str,
        id: &str,
    ) -> Result<(), PersistenceError> {
        let connection = self.connection.lock().expect("database lock poisoned");
        connection.execute(
            "DELETE FROM application_rules WHERE device_model_id = ?1 AND profile_id = ?2 AND capability = 'profile' AND id = ?3",
            params![device_model_id, profile_id, id],
        )?;
        Ok(())
    }

    pub fn load_profile_host_state(
        &self,
        device_model_id: &str,
        profile_id: &str,
    ) -> Result<ProfileHostState, PersistenceError> {
        let connection = self.connection.lock().expect("database lock poisoned");
        connection
            .query_row(
                r#"
                    SELECT live_scene_json, music_config_json, host_mode, lcd_asset_path, lcd_content_hash
                    FROM profile_host_state
                    WHERE device_model_id = ?1 AND profile_id = ?2
                "#,
                params![device_model_id, profile_id],
                |row| {
                    Ok(ProfileHostState {
                        live_scene_json: row.get(0)?,
                        music_config_json: row.get(1)?,
                        host_mode: row.get(2)?,
                        lcd_asset_path: row.get(3)?,
                        lcd_content_hash: row.get(4)?,
                    })
                },
            )
            .optional()
            .map(|state| {
                state.unwrap_or_else(|| ProfileHostState {
                    host_mode: "off".into(),
                    ..ProfileHostState::default()
                })
            })
            .map_err(PersistenceError::from)
    }

    pub fn save_profile_host_state(
        &self,
        device_model_id: &str,
        profile_id: &str,
        state: &ProfileHostState,
    ) -> Result<(), PersistenceError> {
        if !matches!(state.host_mode.as_str(), "off" | "live-rgb" | "music") {
            return Err(PersistenceError::InvalidScope(
                "invalid profile host mode".into(),
            ));
        }
        let connection = self.connection.lock().expect("database lock poisoned");
        connection.execute(
            r#"
                INSERT INTO profile_host_state (
                    device_model_id, profile_id, live_scene_json, music_config_json,
                    host_mode, lcd_asset_path, lcd_content_hash, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                ON CONFLICT(device_model_id, profile_id) DO UPDATE SET
                    live_scene_json = excluded.live_scene_json,
                    music_config_json = excluded.music_config_json,
                    host_mode = excluded.host_mode,
                    lcd_asset_path = excluded.lcd_asset_path,
                    lcd_content_hash = excluded.lcd_content_hash,
                    updated_at = excluded.updated_at
            "#,
            params![
                device_model_id,
                profile_id,
                state.live_scene_json,
                state.music_config_json,
                state.host_mode,
                state.lcd_asset_path,
                state.lcd_content_hash,
                unix_millis()
            ],
        )?;
        Ok(())
    }
}

fn validate_identity(id: &str, name: &str) -> Result<(), PersistenceError> {
    let id_valid = !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'));
    if !id_valid || name.trim().is_empty() || name.trim().len() > 100 {
        return Err(PersistenceError::InvalidScope(
            "profile ID or name is invalid".into(),
        ));
    }
    Ok(())
}

fn ensure_profile(
    transaction: &Transaction<'_>,
    device_model_id: &str,
    profile_id: &str,
) -> Result<(), PersistenceError> {
    let exists = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM profiles WHERE device_model_id = ?1 AND id = ?2)",
        params![device_model_id, profile_id],
        |row| row.get::<_, bool>(0),
    )?;
    if exists {
        Ok(())
    } else {
        Err(PersistenceError::ProfileNotFound {
            device_model_id: device_model_id.into(),
            profile_id: profile_id.into(),
        })
    }
}

fn clone_rows(
    transaction: &Transaction<'_>,
    device_model_id: &str,
    source: &str,
    target: &str,
) -> Result<(), PersistenceError> {
    let statements = [
        "INSERT INTO lighting SELECT device_model_id, ?3, capability, mode, r, g, b, brightness, speed, direction, colorful, ?4 FROM lighting WHERE device_model_id = ?1 AND profile_id = ?2",
        "INSERT INTO device_settings SELECT device_model_id, ?3, capability, setting_key, value_json, ?4 FROM device_settings WHERE device_model_id = ?1 AND profile_id = ?2",
        "INSERT INTO key_mappings SELECT device_model_id, ?3, capability, layer_id, key_id, action_json, ?4 FROM key_mappings WHERE device_model_id = ?1 AND profile_id = ?2",
        "INSERT INTO layer_mappings SELECT device_model_id, ?3, capability, layer_id, name, position, settings_json, ?4 FROM layer_mappings WHERE device_model_id = ?1 AND profile_id = ?2",
        "INSERT INTO macros SELECT device_model_id, ?3, capability, id, name, playback_json, created_at, ?4, hardware_slot FROM macros WHERE device_model_id = ?1 AND profile_id = ?2",
        "INSERT INTO macro_events SELECT device_model_id, ?3, capability, macro_id, event_index, event_json FROM macro_events WHERE device_model_id = ?1 AND profile_id = ?2",
        "INSERT INTO per_key_lighting SELECT device_model_id, ?3, capability, key_id, r, g, b, ?4 FROM per_key_lighting WHERE device_model_id = ?1 AND profile_id = ?2",
        "INSERT INTO live_lighting SELECT device_model_id, ?3, capability, preset_id, name, layers_json, ?4 FROM live_lighting WHERE device_model_id = ?1 AND profile_id = ?2",
        "INSERT INTO music_presets SELECT device_model_id, ?3, capability, preset_id, name, settings_json, ?4 FROM music_presets WHERE device_model_id = ?1 AND profile_id = ?2",
        "INSERT INTO lcd_projects SELECT device_model_id, ?3, capability, project_id, name, source_uri, width, height, frame_count, content_hash, metadata_json, created_at, ?4 FROM lcd_projects WHERE device_model_id = ?1 AND profile_id = ?2",
        "INSERT INTO profile_host_state SELECT device_model_id, ?3, live_scene_json, music_config_json, host_mode, lcd_asset_path, lcd_content_hash, ?4 FROM profile_host_state WHERE device_model_id = ?1 AND profile_id = ?2",
    ];
    let now = unix_millis();
    for statement in statements {
        if statement.contains("?4") {
            transaction.execute(statement, params![device_model_id, source, target, now])?;
        } else {
            transaction.execute(statement, params![device_model_id, source, target])?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mechkeymesh_config::DeviceModel;
    use tempfile::tempdir;

    #[test]
    fn profile_clone_copies_host_state_and_delete_cascades_rules() {
        let directory = tempdir().unwrap();
        let store = PersistenceStore::open(directory.path().join("profiles.sqlite3")).unwrap();
        store
            .ensure_device_model(&DeviceModel::new("aula.f108pro", "AULA", "F108 Pro"))
            .unwrap();
        store.ensure_default_profile("aula.f108pro").unwrap();

        let host = ProfileHostState {
            live_scene_json: Some("{\"layers\":[]}".into()),
            music_config_json: None,
            host_mode: "live-rgb".into(),
            lcd_asset_path: Some("validated.f108lcd".into()),
            lcd_content_hash: Some("0123456789abcdef".into()),
        };
        store
            .save_profile_host_state("aula.f108pro", "default", &host)
            .unwrap();
        let cloned = store
            .clone_profile("aula.f108pro", "default", "gaming", "Gaming")
            .unwrap();
        assert_eq!(cloned.id, "gaming");
        assert_eq!(
            store
                .load_profile_host_state("aula.f108pro", "gaming")
                .unwrap(),
            host
        );

        store
            .save_application_rule(&ApplicationRule {
                id: "game-rule".into(),
                device_model_id: "aula.f108pro".into(),
                profile_id: "gaming".into(),
                executable_pattern: "game*.exe".into(),
                priority: 10,
                enabled: true,
            })
            .unwrap();
        assert_eq!(
            store.list_application_rules("aula.f108pro").unwrap().len(),
            1
        );

        store.delete_profile("aula.f108pro", "gaming").unwrap();
        assert!(
            store
                .list_application_rules("aula.f108pro")
                .unwrap()
                .is_empty()
        );
    }
}
