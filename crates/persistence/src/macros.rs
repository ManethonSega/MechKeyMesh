use super::{PersistenceError, PersistenceStore, unix_millis};
use mechkeymesh_config::{ConfigurationScope, MacroEvent, MacroPlayback, MacroProgram};
use mechkeymesh_driver_api::Capability;
use rusqlite::{TransactionBehavior, params};

impl PersistenceStore {
    pub fn save_macro(
        &self,
        scope: &ConfigurationScope,
        program: &MacroProgram,
    ) -> Result<(), PersistenceError> {
        validate_scope(scope)?;
        program
            .validate()
            .map_err(|error| PersistenceError::Integrity(error.to_string()))?;

        let mut connection = self.connection.lock().expect("database lock poisoned");
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        ensure_profile(&transaction, scope)?;

        let playback_json = serde_json::to_string(&program.playback).map_err(|error| {
            PersistenceError::Integrity(format!("could not serialize macro playback: {error}"))
        })?;
        let now = unix_millis();

        transaction.execute(
            r#"
                INSERT INTO macros (
                    device_model_id,
                    profile_id,
                    capability,
                    id,
                    name,
                    playback_json,
                    created_at,
                    updated_at
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
                ON CONFLICT(device_model_id, profile_id, capability, id)
                DO UPDATE SET
                    name = excluded.name,
                    playback_json = excluded.playback_json,
                    updated_at = excluded.updated_at
            "#,
            params![
                scope.device_model_id,
                scope.profile_id,
                scope.capability.storage_key(),
                program.id,
                program.name,
                playback_json,
                now
            ],
        )?;

        transaction.execute(
            r#"
                DELETE FROM macro_events
                WHERE device_model_id = ?1
                  AND profile_id = ?2
                  AND capability = ?3
                  AND macro_id = ?4
            "#,
            params![
                scope.device_model_id,
                scope.profile_id,
                scope.capability.storage_key(),
                program.id
            ],
        )?;

        for (index, event) in program.events.iter().enumerate() {
            let event_json = serde_json::to_string(event).map_err(|error| {
                PersistenceError::Integrity(format!(
                    "could not serialize macro event {index}: {error}"
                ))
            })?;
            transaction.execute(
                r#"
                    INSERT INTO macro_events (
                        device_model_id,
                        profile_id,
                        capability,
                        macro_id,
                        event_index,
                        event_json
                    )
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                "#,
                params![
                    scope.device_model_id,
                    scope.profile_id,
                    scope.capability.storage_key(),
                    program.id,
                    i64::try_from(index).map_err(|_| PersistenceError::Integrity(
                        "macro event index exceeds SQLite integer range".into()
                    ))?,
                    event_json
                ],
            )?;
        }

        transaction.commit()?;
        Ok(())
    }

    pub fn load_macros(
        &self,
        scope: &ConfigurationScope,
    ) -> Result<Vec<MacroProgram>, PersistenceError> {
        validate_scope(scope)?;
        let connection = self.connection.lock().expect("database lock poisoned");
        let mut macros_statement = connection.prepare(
            r#"
                SELECT id, name, playback_json
                FROM macros
                WHERE device_model_id = ?1
                  AND profile_id = ?2
                  AND capability = ?3
                ORDER BY name COLLATE NOCASE, id
            "#,
        )?;
        let macro_rows = macros_statement.query_map(
            params![
                scope.device_model_id,
                scope.profile_id,
                scope.capability.storage_key()
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )?;

        let mut programs = Vec::new();
        for row in macro_rows {
            let (id, name, playback_json) = row?;
            let playback: MacroPlayback = serde_json::from_str(&playback_json).map_err(|error| {
                PersistenceError::Integrity(format!(
                    "invalid playback JSON for macro {id}: {error}"
                ))
            })?;

            let mut event_statement = connection.prepare(
                r#"
                    SELECT event_json
                    FROM macro_events
                    WHERE device_model_id = ?1
                      AND profile_id = ?2
                      AND capability = ?3
                      AND macro_id = ?4
                    ORDER BY event_index
                "#,
            )?;
            let event_rows = event_statement.query_map(
                params![
                    scope.device_model_id,
                    scope.profile_id,
                    scope.capability.storage_key(),
                    id
                ],
                |event_row| event_row.get::<_, String>(0),
            )?;

            let mut events = Vec::new();
            for event_json in event_rows {
                let event_json = event_json?;
                let event: MacroEvent = serde_json::from_str(&event_json).map_err(|error| {
                    PersistenceError::Integrity(format!(
                        "invalid event JSON for macro {id}: {error}"
                    ))
                })?;
                events.push(event);
            }

            let program = MacroProgram {
                id,
                name,
                playback,
                events,
            };
            program
                .validate()
                .map_err(|error| PersistenceError::Integrity(error.to_string()))?;
            programs.push(program);
        }
        Ok(programs)
    }

    pub fn delete_macro(
        &self,
        scope: &ConfigurationScope,
        macro_id: &str,
    ) -> Result<bool, PersistenceError> {
        validate_scope(scope)?;
        if macro_id.trim().is_empty() {
            return Err(PersistenceError::InvalidScope(
                "macro ID must not be empty".into(),
            ));
        }
        let connection = self.connection.lock().expect("database lock poisoned");
        let affected = connection.execute(
            r#"
                DELETE FROM macros
                WHERE device_model_id = ?1
                  AND profile_id = ?2
                  AND capability = ?3
                  AND id = ?4
            "#,
            params![
                scope.device_model_id,
                scope.profile_id,
                scope.capability.storage_key(),
                macro_id
            ],
        )?;
        Ok(affected != 0)
    }
}

fn validate_scope(scope: &ConfigurationScope) -> Result<(), PersistenceError> {
    if scope.capability != Capability::Macros {
        return Err(PersistenceError::InvalidScope(format!(
            "macros require capability {}, got {}",
            Capability::Macros.storage_key(),
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

fn ensure_profile(
    transaction: &rusqlite::Transaction<'_>,
    scope: &ConfigurationScope,
) -> Result<(), PersistenceError> {
    let profile_exists: bool = transaction.query_row(
        r#"
            SELECT EXISTS(
                SELECT 1 FROM profiles
                WHERE device_model_id = ?1 AND id = ?2
            )
        "#,
        params![scope.device_model_id, scope.profile_id],
        |row| row.get(0),
    )?;
    if !profile_exists {
        return Err(PersistenceError::ProfileNotFound {
            device_model_id: scope.device_model_id.clone(),
            profile_id: scope.profile_id.clone(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mechkeymesh_config::{DeviceModel, MacroMouseButton};
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
        ConfigurationScope::new("aula.f108pro", "default", Capability::Macros)
    }

    fn sample() -> MacroProgram {
        MacroProgram {
            id: "macro-1".into(),
            name: "Copy movement".into(),
            playback: MacroPlayback::Once,
            events: vec![
                MacroEvent::KeyDown { usage: 0x1a },
                MacroEvent::Delay { milliseconds: 83 },
                MacroEvent::KeyUp { usage: 0x1a },
                MacroEvent::MouseDown {
                    button: MacroMouseButton::Left,
                },
                MacroEvent::MouseUp {
                    button: MacroMouseButton::Left,
                },
            ],
        }
    }

    #[test]
    fn macro_and_ordered_events_survive_database_reopen() {
        let (_directory, path) = setup();
        let expected = sample();
        PersistenceStore::open(&path)
            .unwrap()
            .save_macro(&scope(), &expected)
            .unwrap();
        let reopened = PersistenceStore::open(&path).unwrap();
        assert_eq!(reopened.load_macros(&scope()).unwrap(), vec![expected]);
    }

    #[test]
    fn replacement_is_atomic_and_delete_cascades_events() {
        let (_directory, path) = setup();
        let store = PersistenceStore::open(&path).unwrap();
        let original = sample();
        store.save_macro(&scope(), &original).unwrap();

        let mut replacement = original.clone();
        replacement.name = "Renamed".into();
        replacement.events = vec![MacroEvent::KeyDown { usage: 6 }];
        store.save_macro(&scope(), &replacement).unwrap();
        assert_eq!(store.load_macros(&scope()).unwrap(), vec![replacement]);

        assert!(store.delete_macro(&scope(), "macro-1").unwrap());
        assert!(store.load_macros(&scope()).unwrap().is_empty());
        assert!(!store.delete_macro(&scope(), "macro-1").unwrap());
    }

    #[test]
    fn wrong_scope_and_invalid_event_are_rejected() {
        let (_directory, path) = setup();
        let store = PersistenceStore::open(&path).unwrap();
        let wrong = ConfigurationScope::new(
            "aula.f108pro",
            "default",
            Capability::DeviceSettings,
        );
        assert!(store.save_macro(&wrong, &sample()).is_err());

        let mut invalid = sample();
        invalid.events.push(MacroEvent::KeyDown { usage: 0 });
        assert!(store.save_macro(&scope(), &invalid).is_err());
    }
}
