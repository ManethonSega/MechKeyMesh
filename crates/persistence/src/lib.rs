use mechkeymesh_config::{ConfigurationScope, DesiredLighting, DeviceModel, Profile};
use mechkeymesh_driver_api::{Capability, LightingConfig};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

pub const CURRENT_SCHEMA_VERSION: i64 = 4;

struct Migration {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "foundation",
        sql: r#"
            CREATE TABLE devices (
                id TEXT PRIMARY KEY NOT NULL,
                vendor TEXT NOT NULL,
                name TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );

            CREATE TABLE profiles (
                device_model_id TEXT NOT NULL,
                id TEXT NOT NULL,
                name TEXT NOT NULL,
                is_default INTEGER NOT NULL DEFAULT 0 CHECK (is_default IN (0, 1)),
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (device_model_id, id),
                FOREIGN KEY (device_model_id)
                    REFERENCES devices(id)
                    ON DELETE CASCADE
            );

            CREATE UNIQUE INDEX profiles_one_default_per_device
                ON profiles(device_model_id)
                WHERE is_default = 1;

            CREATE TABLE lighting (
                device_model_id TEXT NOT NULL,
                profile_id TEXT NOT NULL,
                capability TEXT NOT NULL,
                mode INTEGER NOT NULL,
                r INTEGER NOT NULL,
                g INTEGER NOT NULL,
                b INTEGER NOT NULL,
                brightness INTEGER NOT NULL,
                speed INTEGER NOT NULL,
                direction INTEGER NOT NULL,
                colorful INTEGER NOT NULL CHECK (colorful IN (0, 1)),
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (device_model_id, profile_id, capability),
                FOREIGN KEY (device_model_id, profile_id)
                    REFERENCES profiles(device_model_id, id)
                    ON DELETE CASCADE
            );

            CREATE TABLE device_settings (
                device_model_id TEXT NOT NULL,
                profile_id TEXT NOT NULL,
                capability TEXT NOT NULL,
                setting_key TEXT NOT NULL,
                value_json TEXT NOT NULL,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (
                    device_model_id,
                    profile_id,
                    capability,
                    setting_key
                ),
                FOREIGN KEY (device_model_id, profile_id)
                    REFERENCES profiles(device_model_id, id)
                    ON DELETE CASCADE
            );

            CREATE TABLE application_rules (
                device_model_id TEXT NOT NULL,
                profile_id TEXT NOT NULL,
                capability TEXT NOT NULL,
                id TEXT NOT NULL,
                executable_pattern TEXT NOT NULL,
                priority INTEGER NOT NULL DEFAULT 0,
                enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (
                    device_model_id,
                    profile_id,
                    capability,
                    id
                ),
                FOREIGN KEY (device_model_id, profile_id)
                    REFERENCES profiles(device_model_id, id)
                    ON DELETE CASCADE
            );

            CREATE INDEX application_rules_lookup
                ON application_rules(enabled, priority DESC);
        "#,
    },
    Migration {
        version: 2,
        name: "keymaps-and-macros",
        sql: r#"
            CREATE TABLE key_mappings (
                device_model_id TEXT NOT NULL,
                profile_id TEXT NOT NULL,
                capability TEXT NOT NULL,
                layer_id TEXT NOT NULL,
                key_id TEXT NOT NULL,
                action_json TEXT NOT NULL,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (
                    device_model_id,
                    profile_id,
                    capability,
                    layer_id,
                    key_id
                ),
                FOREIGN KEY (device_model_id, profile_id)
                    REFERENCES profiles(device_model_id, id)
                    ON DELETE CASCADE
            );

            CREATE TABLE layer_mappings (
                device_model_id TEXT NOT NULL,
                profile_id TEXT NOT NULL,
                capability TEXT NOT NULL,
                layer_id TEXT NOT NULL,
                name TEXT NOT NULL,
                position INTEGER NOT NULL,
                settings_json TEXT NOT NULL DEFAULT '{}',
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (
                    device_model_id,
                    profile_id,
                    capability,
                    layer_id
                ),
                FOREIGN KEY (device_model_id, profile_id)
                    REFERENCES profiles(device_model_id, id)
                    ON DELETE CASCADE
            );

            CREATE TABLE macros (
                device_model_id TEXT NOT NULL,
                profile_id TEXT NOT NULL,
                capability TEXT NOT NULL,
                id TEXT NOT NULL,
                name TEXT NOT NULL,
                playback_json TEXT NOT NULL DEFAULT '{}',
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (
                    device_model_id,
                    profile_id,
                    capability,
                    id
                ),
                FOREIGN KEY (device_model_id, profile_id)
                    REFERENCES profiles(device_model_id, id)
                    ON DELETE CASCADE
            );

            CREATE TABLE macro_events (
                device_model_id TEXT NOT NULL,
                profile_id TEXT NOT NULL,
                capability TEXT NOT NULL,
                macro_id TEXT NOT NULL,
                event_index INTEGER NOT NULL,
                event_json TEXT NOT NULL,
                PRIMARY KEY (
                    device_model_id,
                    profile_id,
                    capability,
                    macro_id,
                    event_index
                ),
                FOREIGN KEY (
                    device_model_id,
                    profile_id,
                    capability,
                    macro_id
                )
                    REFERENCES macros(
                        device_model_id,
                        profile_id,
                        capability,
                        id
                    )
                    ON DELETE CASCADE
            );
        "#,
    },
    Migration {
        version: 3,
        name: "lighting-media-and-presets",
        sql: r#"
            CREATE TABLE per_key_lighting (
                device_model_id TEXT NOT NULL,
                profile_id TEXT NOT NULL,
                capability TEXT NOT NULL,
                key_id TEXT NOT NULL,
                r INTEGER NOT NULL,
                g INTEGER NOT NULL,
                b INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (
                    device_model_id,
                    profile_id,
                    capability,
                    key_id
                ),
                FOREIGN KEY (device_model_id, profile_id)
                    REFERENCES profiles(device_model_id, id)
                    ON DELETE CASCADE
            );

            CREATE TABLE live_lighting (
                device_model_id TEXT NOT NULL,
                profile_id TEXT NOT NULL,
                capability TEXT NOT NULL,
                preset_id TEXT NOT NULL,
                name TEXT NOT NULL,
                layers_json TEXT NOT NULL,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (
                    device_model_id,
                    profile_id,
                    capability,
                    preset_id
                ),
                FOREIGN KEY (device_model_id, profile_id)
                    REFERENCES profiles(device_model_id, id)
                    ON DELETE CASCADE
            );

            CREATE TABLE music_presets (
                device_model_id TEXT NOT NULL,
                profile_id TEXT NOT NULL,
                capability TEXT NOT NULL,
                preset_id TEXT NOT NULL,
                name TEXT NOT NULL,
                settings_json TEXT NOT NULL,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (
                    device_model_id,
                    profile_id,
                    capability,
                    preset_id
                ),
                FOREIGN KEY (device_model_id, profile_id)
                    REFERENCES profiles(device_model_id, id)
                    ON DELETE CASCADE
            );

            CREATE TABLE lcd_projects (
                device_model_id TEXT NOT NULL,
                profile_id TEXT NOT NULL,
                capability TEXT NOT NULL,
                project_id TEXT NOT NULL,
                name TEXT NOT NULL,
                source_uri TEXT,
                width INTEGER NOT NULL,
                height INTEGER NOT NULL,
                frame_count INTEGER NOT NULL,
                content_hash TEXT,
                metadata_json TEXT NOT NULL DEFAULT '{}',
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (
                    device_model_id,
                    profile_id,
                    capability,
                    project_id
                ),
                FOREIGN KEY (device_model_id, profile_id)
                    REFERENCES profiles(device_model_id, id)
                    ON DELETE CASCADE
            );
        "#,
    },
    Migration {
        version: 4,
        name: "stable-macro-hardware-slots",
        sql: r#"
            ALTER TABLE macros ADD COLUMN hardware_slot INTEGER
                CHECK (hardware_slot IS NULL OR hardware_slot BETWEEN 0 AND 99);

            UPDATE macros
            SET hardware_slot = (
                SELECT COUNT(*) - 1
                FROM macros AS preceding
                WHERE preceding.device_model_id = macros.device_model_id
                  AND preceding.profile_id = macros.profile_id
                  AND preceding.capability = macros.capability
                  AND preceding.id <= macros.id
            )
            WHERE (
                SELECT COUNT(*)
                FROM macros AS preceding
                WHERE preceding.device_model_id = macros.device_model_id
                  AND preceding.profile_id = macros.profile_id
                  AND preceding.capability = macros.capability
                  AND preceding.id <= macros.id
            ) <= 100;

            CREATE UNIQUE INDEX macros_one_hardware_slot
                ON macros(device_model_id, profile_id, capability, hardware_slot)
                WHERE hardware_slot IS NOT NULL;
        "#,
    },
];

#[derive(Debug, Error)]
pub enum PersistenceError {
    #[error("database I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("database schema {found} is newer than this MechKeyMesh build supports ({supported})")]
    NewerSchema { found: i64, supported: i64 },
    #[error("database integrity check failed: {0}")]
    Integrity(String),
    #[error("invalid configuration scope: {0}")]
    InvalidScope(String),
    #[error("profile not found for device model {device_model_id}: {profile_id}")]
    ProfileNotFound {
        device_model_id: String,
        profile_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistenceStatus {
    pub path: String,
    pub schema_version: i64,
    pub journal_mode: String,
    pub integrity_ok: bool,
}

pub struct PersistenceStore {
    path: PathBuf,
    connection: Mutex<Connection>,
}

impl PersistenceStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, PersistenceError> {
        Self::open_to_schema(path.as_ref(), CURRENT_SCHEMA_VERSION)
    }

    fn open_to_schema(path: &Path, target_schema: i64) -> Result<Self, PersistenceError> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)?;
        }

        let mut connection = Connection::open(path)?;
        configure_connection(&connection)?;
        migrate(&mut connection, target_schema)?;
        verify_integrity(&connection)?;

        Ok(Self {
            path: path.to_path_buf(),
            connection: Mutex::new(connection),
        })
    }

    pub fn status(&self) -> Result<PersistenceStatus, PersistenceError> {
        let connection = self.connection.lock().expect("database lock poisoned");
        let schema_version = schema_version(&connection)?;
        let journal_mode =
            connection.query_row("PRAGMA journal_mode", [], |row| row.get::<_, String>(0))?;
        verify_integrity(&connection)?;

        Ok(PersistenceStatus {
            path: self.path.to_string_lossy().into_owned(),
            schema_version,
            journal_mode,
            integrity_ok: true,
        })
    }

    pub fn ensure_device_model(&self, model: &DeviceModel) -> Result<(), PersistenceError> {
        let now = unix_millis();
        let connection = self.connection.lock().expect("database lock poisoned");
        connection.execute(
            r#"
                INSERT INTO devices (
                    id,
                    vendor,
                    name,
                    created_at,
                    updated_at
                )
                VALUES (?1, ?2, ?3, ?4, ?4)
                ON CONFLICT(id) DO UPDATE SET
                    vendor = excluded.vendor,
                    name = excluded.name,
                    updated_at = excluded.updated_at
            "#,
            params![model.id, model.vendor, model.name, now],
        )?;
        Ok(())
    }

    pub fn ensure_default_profile(
        &self,
        device_model_id: &str,
    ) -> Result<Profile, PersistenceError> {
        let mut connection = self.connection.lock().expect("database lock poisoned");
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

        let existing = transaction
            .query_row(
                r#"
                    SELECT id, name
                    FROM profiles
                    WHERE device_model_id = ?1
                      AND is_default = 1
                    LIMIT 1
                "#,
                [device_model_id],
                |row| {
                    Ok(Profile {
                        id: row.get(0)?,
                        device_model_id: device_model_id.to_string(),
                        name: row.get(1)?,
                        is_default: true,
                    })
                },
            )
            .optional()?;

        if let Some(profile) = existing {
            transaction.commit()?;
            return Ok(profile);
        }

        let now = unix_millis();
        transaction.execute(
            r#"
                INSERT INTO profiles (
                    device_model_id,
                    id,
                    name,
                    is_default,
                    created_at,
                    updated_at
                )
                VALUES (?1, 'default', 'Default', 1, ?2, ?2)
            "#,
            params![device_model_id, now],
        )?;
        transaction.commit()?;

        Ok(Profile {
            id: "default".into(),
            device_model_id: device_model_id.into(),
            name: "Default".into(),
            is_default: true,
        })
    }

    pub fn default_profile(
        &self,
        device_model_id: &str,
    ) -> Result<Option<Profile>, PersistenceError> {
        let connection = self.connection.lock().expect("database lock poisoned");
        connection
            .query_row(
                r#"
                    SELECT id, name
                    FROM profiles
                    WHERE device_model_id = ?1
                      AND is_default = 1
                    LIMIT 1
                "#,
                [device_model_id],
                |row| {
                    Ok(Profile {
                        id: row.get(0)?,
                        device_model_id: device_model_id.to_string(),
                        name: row.get(1)?,
                        is_default: true,
                    })
                },
            )
            .optional()
            .map_err(PersistenceError::from)
    }

    pub fn save_lighting(&self, desired: &DesiredLighting) -> Result<(), PersistenceError> {
        validate_lighting_scope(&desired.scope)?;
        let now = unix_millis();
        let capability = desired.scope.capability.storage_key();
        let config = desired.config;

        let connection = self.connection.lock().expect("database lock poisoned");
        let changed = connection.execute(
            r#"
                INSERT INTO lighting (
                    device_model_id,
                    profile_id,
                    capability,
                    mode,
                    r,
                    g,
                    b,
                    brightness,
                    speed,
                    direction,
                    colorful,
                    updated_at
                )
                VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12
                )
                ON CONFLICT(
                    device_model_id,
                    profile_id,
                    capability
                ) DO UPDATE SET
                    mode = excluded.mode,
                    r = excluded.r,
                    g = excluded.g,
                    b = excluded.b,
                    brightness = excluded.brightness,
                    speed = excluded.speed,
                    direction = excluded.direction,
                    colorful = excluded.colorful,
                    updated_at = excluded.updated_at
            "#,
            params![
                desired.scope.device_model_id,
                desired.scope.profile_id,
                capability,
                i64::from(config.mode),
                i64::from(config.r),
                i64::from(config.g),
                i64::from(config.b),
                i64::from(config.brightness),
                i64::from(config.speed),
                i64::from(config.direction),
                i64::from(config.colorful),
                now
            ],
        );

        match changed {
            Ok(_) => Ok(()),
            Err(rusqlite::Error::SqliteFailure(error, _))
                if error.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_FOREIGNKEY =>
            {
                Err(PersistenceError::ProfileNotFound {
                    device_model_id: desired.scope.device_model_id.clone(),
                    profile_id: desired.scope.profile_id.clone(),
                })
            }
            Err(error) => Err(PersistenceError::Sqlite(error)),
        }
    }

    pub fn load_lighting(
        &self,
        scope: &ConfigurationScope,
    ) -> Result<Option<DesiredLighting>, PersistenceError> {
        validate_lighting_scope(scope)?;
        let connection = self.connection.lock().expect("database lock poisoned");

        connection
            .query_row(
                r#"
                    SELECT
                        mode,
                        r,
                        g,
                        b,
                        brightness,
                        speed,
                        direction,
                        colorful
                    FROM lighting
                    WHERE device_model_id = ?1
                      AND profile_id = ?2
                      AND capability = ?3
                "#,
                params![
                    scope.device_model_id,
                    scope.profile_id,
                    scope.capability.storage_key()
                ],
                |row| {
                    Ok(DesiredLighting {
                        scope: scope.clone(),
                        config: LightingConfig {
                            mode: row.get::<_, u8>(0)?,
                            r: row.get::<_, u8>(1)?,
                            g: row.get::<_, u8>(2)?,
                            b: row.get::<_, u8>(3)?,
                            brightness: row.get::<_, u8>(4)?,
                            speed: row.get::<_, u8>(5)?,
                            direction: row.get::<_, u8>(6)?,
                            colorful: row.get::<_, bool>(7)?,
                        },
                    })
                },
            )
            .optional()
            .map_err(PersistenceError::from)
    }
}

fn configure_connection(connection: &Connection) -> Result<(), PersistenceError> {
    connection.busy_timeout(Duration::from_secs(5))?;
    connection.execute_batch(
        r#"
            PRAGMA foreign_keys = ON;
            PRAGMA synchronous = FULL;
        "#,
    )?;
    let _: String = connection.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
    Ok(())
}

fn migrate(connection: &mut Connection, target_schema: i64) -> Result<(), PersistenceError> {
    connection.execute_batch(
        r#"
            CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                applied_at INTEGER NOT NULL
            );
        "#,
    )?;

    let current = schema_version(connection)?;
    if current > target_schema {
        return Err(PersistenceError::NewerSchema {
            found: current,
            supported: target_schema,
        });
    }

    for migration in MIGRATIONS
        .iter()
        .filter(|migration| migration.version > current && migration.version <= target_schema)
    {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute_batch(migration.sql)?;
        transaction.execute(
            r#"
                INSERT INTO schema_migrations(version, name, applied_at)
                VALUES (?1, ?2, ?3)
            "#,
            params![migration.version, migration.name, unix_millis()],
        )?;
        transaction.pragma_update(None, "user_version", migration.version)?;
        transaction.commit()?;
    }

    Ok(())
}

fn schema_version(connection: &Connection) -> Result<i64, PersistenceError> {
    connection
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )
        .map_err(PersistenceError::from)
}

fn verify_integrity(connection: &Connection) -> Result<(), PersistenceError> {
    let result: String = connection.query_row("PRAGMA quick_check", [], |row| row.get(0))?;

    if result.eq_ignore_ascii_case("ok") {
        Ok(())
    } else {
        Err(PersistenceError::Integrity(result))
    }
}

fn validate_lighting_scope(scope: &ConfigurationScope) -> Result<(), PersistenceError> {
    if scope.capability != Capability::HardwareRgb {
        return Err(PersistenceError::InvalidScope(format!(
            "lighting records require capability {}, got {}",
            Capability::HardwareRgb.storage_key(),
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

fn unix_millis() -> i64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    i64::try_from(millis).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    const EXPECTED_TABLES: &[&str] = &[
        "devices",
        "profiles",
        "key_mappings",
        "layer_mappings",
        "macros",
        "macro_events",
        "lighting",
        "per_key_lighting",
        "live_lighting",
        "music_presets",
        "lcd_projects",
        "device_settings",
        "application_rules",
    ];

    fn model() -> DeviceModel {
        DeviceModel::new("aula.f108pro", "AULA", "AULA F108 Pro")
    }

    fn lighting(scope: ConfigurationScope) -> DesiredLighting {
        DesiredLighting::new(
            scope,
            LightingConfig {
                mode: 7,
                r: 12,
                g: 34,
                b: 56,
                brightness: 5,
                speed: 3,
                direction: 1,
                colorful: true,
            },
        )
    }

    #[test]
    fn new_database_contains_all_core_objects() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("mechkeymesh.sqlite3");
        let store = PersistenceStore::open(&path).unwrap();

        assert_eq!(
            store.status().unwrap().schema_version,
            CURRENT_SCHEMA_VERSION
        );

        let connection = store.connection.lock().unwrap();
        for table in EXPECTED_TABLES {
            let exists: i64 = connection
                .query_row(
                    r#"
                        SELECT COUNT(*)
                        FROM sqlite_master
                        WHERE type = 'table'
                          AND name = ?1
                    "#,
                    [table],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(exists, 1, "missing table {table}");
        }
    }

    #[test]
    fn desired_lighting_survives_database_reopen() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("mechkeymesh.sqlite3");
        let expected;

        {
            let store = PersistenceStore::open(&path).unwrap();
            store.ensure_device_model(&model()).unwrap();
            let profile = store.ensure_default_profile("aula.f108pro").unwrap();
            let scope =
                ConfigurationScope::new("aula.f108pro", profile.id, Capability::HardwareRgb);
            expected = lighting(scope.clone());
            store.save_lighting(&expected).unwrap();
            assert_eq!(store.load_lighting(&scope).unwrap(), Some(expected.clone()));
        }

        let reopened = PersistenceStore::open(&path).unwrap();
        assert_eq!(
            reopened.load_lighting(&expected.scope).unwrap(),
            Some(expected)
        );
        assert!(reopened.status().unwrap().integrity_ok);
    }

    #[test]
    fn schema_one_upgrades_without_losing_existing_configuration() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("mechkeymesh.sqlite3");
        let expected;

        {
            let store = PersistenceStore::open_to_schema(&path, 1).unwrap();
            assert_eq!(store.status().unwrap().schema_version, 1);
            store.ensure_device_model(&model()).unwrap();
            let profile = store.ensure_default_profile("aula.f108pro").unwrap();
            let scope =
                ConfigurationScope::new("aula.f108pro", profile.id, Capability::HardwareRgb);
            expected = lighting(scope);
            store.save_lighting(&expected).unwrap();
        }

        let upgraded = PersistenceStore::open(&path).unwrap();
        assert_eq!(
            upgraded.status().unwrap().schema_version,
            CURRENT_SCHEMA_VERSION
        );
        assert_eq!(
            upgraded.load_lighting(&expected.scope).unwrap(),
            Some(expected)
        );
    }

    #[test]
    fn lighting_requires_hardware_rgb_scope() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("mechkeymesh.sqlite3");
        let store = PersistenceStore::open(path).unwrap();

        let scope = ConfigurationScope::new("aula.f108pro", "default", Capability::Macros);
        let error = store.save_lighting(&lighting(scope)).unwrap_err();
        assert!(matches!(error, PersistenceError::InvalidScope(_)));
    }

    #[test]
    fn missing_profile_is_rejected_by_foreign_key() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("mechkeymesh.sqlite3");
        let store = PersistenceStore::open(path).unwrap();
        store.ensure_device_model(&model()).unwrap();

        let scope =
            ConfigurationScope::new("aula.f108pro", "does-not-exist", Capability::HardwareRgb);
        let error = store.save_lighting(&lighting(scope)).unwrap_err();
        assert!(matches!(error, PersistenceError::ProfileNotFound { .. }));
    }
}
