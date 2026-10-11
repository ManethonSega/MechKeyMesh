use super::{PersistenceError, PersistenceStore, unix_millis};
use mechkeymesh_config::{ConfigurationScope, DesiredKeymap, KeyMapping, layer_id};
use mechkeymesh_driver_api::{Capability, KeyAction, KeymapLayer};
use rusqlite::{TransactionBehavior, params};
use std::collections::HashSet;

impl PersistenceStore {
    pub fn save_keymap(&self, desired: &DesiredKeymap) -> Result<(), PersistenceError> {
        self.save_keymaps(std::slice::from_ref(desired))
    }

    pub fn save_keymaps(&self, desired_keymaps: &[DesiredKeymap]) -> Result<(), PersistenceError> {
        for desired in desired_keymaps {
            validate_keymap_scope(desired)?;
        }
        if desired_keymaps.is_empty() {
            return Ok(());
        }

        let mut connection = self.connection.lock().expect("database lock poisoned");
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let now = unix_millis();

        for desired in desired_keymaps {
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

            transaction.execute(
                r#"
                    DELETE FROM key_mappings
                    WHERE device_model_id = ?1
                      AND profile_id = ?2
                      AND capability = ?3
                      AND layer_id = ?4
                "#,
                params![
                    desired.scope.device_model_id,
                    desired.scope.profile_id,
                    desired.scope.capability.storage_key(),
                    desired.layer_id
                ],
            )?;

            for mapping in &desired.mappings {
                if mapping.action == KeyAction::Default {
                    continue;
                }

                let action_json = serde_json::to_string(&mapping.action).map_err(|error| {
                    PersistenceError::Integrity(format!(
                        "could not serialize key action for {}: {error}",
                        mapping.key_id
                    ))
                })?;

                transaction.execute(
                    r#"
                        INSERT INTO key_mappings (
                            device_model_id,
                            profile_id,
                            capability,
                            layer_id,
                            key_id,
                            action_json,
                            updated_at
                        )
                        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                    "#,
                    params![
                        desired.scope.device_model_id,
                        desired.scope.profile_id,
                        desired.scope.capability.storage_key(),
                        desired.layer_id,
                        mapping.key_id,
                        action_json,
                        now
                    ],
                )?;
            }
        }

        transaction.commit()?;
        Ok(())
    }

    pub fn load_keymap(
        &self,
        scope: &ConfigurationScope,
        layer: KeymapLayer,
    ) -> Result<DesiredKeymap, PersistenceError> {
        validate_scope(scope, layer)?;
        let layer_id = layer_id(layer);
        let connection = self.connection.lock().expect("database lock poisoned");
        let mut statement = connection.prepare(
            r#"
                SELECT key_id, action_json
                FROM key_mappings
                WHERE device_model_id = ?1
                  AND profile_id = ?2
                  AND capability = ?3
                  AND layer_id = ?4
                ORDER BY key_id
            "#,
        )?;

        let rows = statement.query_map(
            params![
                scope.device_model_id,
                scope.profile_id,
                scope.capability.storage_key(),
                layer_id
            ],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )?;

        let mut mappings = Vec::new();
        for row in rows {
            let (key_id, action_json) = row?;
            let action: KeyAction = serde_json::from_str(&action_json).map_err(|error| {
                PersistenceError::Integrity(format!(
                    "invalid key action JSON for {key_id}: {error}"
                ))
            })?;
            mappings.push(KeyMapping { key_id, action });
        }

        Ok(DesiredKeymap {
            scope: scope.clone(),
            layer,
            layer_id: layer_id.into(),
            mappings,
        })
    }
}

fn validate_keymap_scope(desired: &DesiredKeymap) -> Result<(), PersistenceError> {
    validate_scope(&desired.scope, desired.layer)?;
    if desired.layer_id != layer_id(desired.layer) {
        return Err(PersistenceError::InvalidScope(format!(
            "layer ID {} does not match {:?}",
            desired.layer_id, desired.layer
        )));
    }

    let mut seen = HashSet::new();
    for mapping in &desired.mappings {
        if mapping.key_id.trim().is_empty() {
            return Err(PersistenceError::InvalidScope(
                "key mapping ID must not be empty".into(),
            ));
        }
        if !seen.insert(mapping.key_id.as_str()) {
            return Err(PersistenceError::InvalidScope(format!(
                "duplicate key mapping: {}",
                mapping.key_id
            )));
        }
    }
    Ok(())
}

fn validate_scope(scope: &ConfigurationScope, layer: KeymapLayer) -> Result<(), PersistenceError> {
    let expected_capability = match layer {
        KeymapLayer::Normal => Capability::Keys,
        KeymapLayer::Fn => Capability::FnLayer,
    };
    if scope.capability != expected_capability {
        return Err(PersistenceError::InvalidScope(format!(
            "{layer:?} key mappings require capability {}, got {}",
            expected_capability.storage_key(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use mechkeymesh_config::DeviceModel;
    use mechkeymesh_driver_api::{Modifier, WindowsAction};
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

    fn normal_scope() -> ConfigurationScope {
        ConfigurationScope::new("aula.f108pro", "default", Capability::Keys)
    }

    fn fn_scope() -> ConfigurationScope {
        ConfigurationScope::new("aula.f108pro", "default", Capability::FnLayer)
    }

    #[test]
    fn keymap_survives_database_reopen() {
        let (_directory, path) = setup();
        let desired = DesiredKeymap::normal(
            normal_scope(),
            vec![
                KeyMapping::new("key-055", KeyAction::Disabled),
                KeyMapping::new(
                    "key-002",
                    KeyAction::ModifiedKey {
                        modifiers: vec![Modifier::LeftCtrl],
                        key: 0x06,
                    },
                ),
            ],
        );

        PersistenceStore::open(&path)
            .unwrap()
            .save_keymap(&desired)
            .unwrap();

        let reopened = PersistenceStore::open(&path).unwrap();
        assert_eq!(
            reopened
                .load_keymap(&normal_scope(), KeymapLayer::Normal)
                .unwrap(),
            desired
        );
    }

    #[test]
    fn fn_keymap_survives_database_reopen_independently() {
        let (_directory, path) = setup();
        let normal = DesiredKeymap::normal(
            normal_scope(),
            vec![KeyMapping::new("key-055", KeyAction::Disabled)],
        );
        let fn_layer = DesiredKeymap::fn_layer(
            fn_scope(),
            vec![KeyMapping::new(
                "key-002",
                KeyAction::ModifiedKey {
                    modifiers: vec![Modifier::LeftCtrl],
                    key: 0x06,
                },
            )],
        );

        let store = PersistenceStore::open(&path).unwrap();
        store
            .save_keymaps(&[normal.clone(), fn_layer.clone()])
            .unwrap();
        drop(store);

        let reopened = PersistenceStore::open(&path).unwrap();
        assert_eq!(
            reopened
                .load_keymap(&normal_scope(), KeymapLayer::Normal)
                .unwrap(),
            normal
        );
        assert_eq!(
            reopened.load_keymap(&fn_scope(), KeymapLayer::Fn).unwrap(),
            fn_layer
        );
    }

    #[test]
    fn multi_layer_replacement_rolls_back_as_one_transaction() {
        let (_directory, path) = setup();
        let store = PersistenceStore::open(&path).unwrap();
        let original = DesiredKeymap::normal(
            normal_scope(),
            vec![KeyMapping::new("key-001", KeyAction::Disabled)],
        );
        store.save_keymap(&original).unwrap();

        let clear_normal = DesiredKeymap::normal(normal_scope(), vec![]);
        let missing_fn = DesiredKeymap::fn_layer(
            ConfigurationScope::new("aula.f108pro", "missing", Capability::FnLayer),
            vec![],
        );
        assert!(store.save_keymaps(&[clear_normal, missing_fn]).is_err());

        assert_eq!(
            store
                .load_keymap(&normal_scope(), KeymapLayer::Normal)
                .unwrap(),
            original
        );
    }

    #[test]
    fn replacement_is_atomic_and_removes_old_mappings() {
        let (_directory, path) = setup();
        let store = PersistenceStore::open(&path).unwrap();
        store
            .save_keymap(&DesiredKeymap::normal(
                normal_scope(),
                vec![
                    KeyMapping::new("key-001", KeyAction::Disabled),
                    KeyMapping::new(
                        "key-002",
                        KeyAction::Windows {
                            action: WindowsAction::Calculator,
                        },
                    ),
                ],
            ))
            .unwrap();

        let replacement = DesiredKeymap::normal(
            normal_scope(),
            vec![KeyMapping::new("key-003", KeyAction::Disabled)],
        );
        store.save_keymap(&replacement).unwrap();

        assert_eq!(
            store
                .load_keymap(&normal_scope(), KeymapLayer::Normal)
                .unwrap(),
            replacement
        );
    }

    #[test]
    fn default_actions_are_not_stored() {
        let (_directory, path) = setup();
        let store = PersistenceStore::open(&path).unwrap();
        store
            .save_keymap(&DesiredKeymap::normal(
                normal_scope(),
                vec![KeyMapping::new("key-001", KeyAction::Default)],
            ))
            .unwrap();

        assert!(
            store
                .load_keymap(&normal_scope(), KeymapLayer::Normal)
                .unwrap()
                .mappings
                .is_empty()
        );
    }

    #[test]
    fn rejects_wrong_capability_and_duplicate_keys() {
        let (_directory, path) = setup();
        let store = PersistenceStore::open(&path).unwrap();
        let wrong_scope =
            ConfigurationScope::new("aula.f108pro", "default", Capability::HardwareRgb);
        assert!(
            store
                .save_keymap(&DesiredKeymap::normal(wrong_scope, vec![]))
                .is_err()
        );

        let duplicate = DesiredKeymap::normal(
            normal_scope(),
            vec![
                KeyMapping::new("key-001", KeyAction::Disabled),
                KeyMapping::new("key-001", KeyAction::Disabled),
            ],
        );
        assert!(store.save_keymap(&duplicate).is_err());
    }

    #[test]
    fn rejects_missing_profile_even_for_empty_map() {
        let (_directory, path) = setup();
        let store = PersistenceStore::open(&path).unwrap();
        let missing = ConfigurationScope::new("aula.f108pro", "missing", Capability::Keys);
        let error = store
            .save_keymap(&DesiredKeymap::normal(missing, vec![]))
            .unwrap_err();
        assert!(matches!(error, PersistenceError::ProfileNotFound { .. }));
    }
}
