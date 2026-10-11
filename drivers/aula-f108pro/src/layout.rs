use mechkeymesh_layout::Layout;

const F108_ANSI_LAYOUT: &str = include_str!("../layouts/f108pro-ansi.json");

#[cfg(test)]
const OEM_LAYOUT_SHA256: &str =
    "7c3f0720bfe97b9ef1dd820e86fb1e621ff20a9a0eeaf6d97b4329a3aa3c3c62";

pub fn f108pro_layout() -> Layout {
    let layout: Layout = serde_json::from_str(F108_ANSI_LAYOUT)
        .expect("embedded F108 Pro layout must be valid JSON");
    layout
        .validate()
        .expect("embedded F108 Pro layout must pass layout validation");
    layout
}

#[cfg(test)]
mod tests {
    use super::*;
    use mechkeymesh_layout::SUPPORTED_LAYOUT_SCHEMA_VERSION;

    #[test]
    fn embedded_f108_layout_is_valid_and_complete() {
        let layout = f108pro_layout();
        assert_eq!(layout.schema_version, SUPPORTED_LAYOUT_SCHEMA_VERSION);
        assert_eq!(layout.id, "aula.f108pro.ansi");
        assert_eq!(layout.device_model_id, "aula.f108pro");
        assert_eq!(layout.keys.len(), 104);
        assert!(layout.keys.iter().all(|key| key.matrix.is_none()));
        assert!(layout.keys.iter().all(|key| key.grid_position.is_some()));
        assert!(layout.keys.iter().all(|key| key.hid_usage.is_some()));
        assert!(layout.keys.iter().all(|key| key.device_key_index.is_some()));
        assert!(layout.keys.iter().all(|key| key.led_index.is_some()));
        assert!(layout.keys.iter().all(|key| key.source_label.is_some()));
        assert!(layout.keys.iter().all(|key| key.fn_programmable));
    }

    #[test]
    fn f108_source_fingerprint_is_preserved() {
        let layout = f108pro_layout();
        assert!(layout.source_notes.iter().any(|note| note.contains(OEM_LAYOUT_SHA256)));
    }

    #[test]
    fn source_grid_defaults_to_unit_zero_and_is_unique() {
        let layout = f108pro_layout();
        let mut positions = std::collections::HashSet::new();

        for key in &layout.keys {
            let grid = key.grid_position.expect("F108 key must have source-grid data");
            assert_eq!(grid.unit, 0);
            assert!(positions.insert(grid), "duplicate source-grid position for {}", key.id);
        }
    }

    #[test]
    fn f108_geometry_and_indices_match_known_reference_keys() {
        let layout = f108pro_layout();

        let escape = layout.key("key-001").unwrap();
        assert_eq!(escape.label, "Esc");
        assert_eq!(escape.x, 27.0);
        assert_eq!(escape.y, 52.0);
        assert_eq!(escape.device_key_index, Some(1));
        assert_eq!(escape.led_index, Some(1));

        let space = layout.key("key-094").unwrap();
        assert_eq!(space.label, "Space");
        assert_eq!(space.width, 204.0);
        assert_eq!(space.device_key_index, Some(94));

        let numpad_enter = layout.key("key-106").unwrap();
        assert_eq!(numpad_enter.height, 59.0);
        assert_eq!(numpad_enter.led_index, Some(106));
    }
}
