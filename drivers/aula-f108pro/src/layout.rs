use mechkeymesh_layout::Layout;

const F108_ANSI_LAYOUT: &str = include_str!("../layouts/f108pro-ansi.json");

pub fn f108pro_layout() -> Layout {
    let layout: Layout =
        serde_json::from_str(F108_ANSI_LAYOUT).expect("embedded F108 Pro layout must be valid JSON");
    layout
        .validate()
        .expect("embedded F108 Pro layout must pass layout validation");
    layout
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_f108_layout_is_valid_and_complete() {
        let layout = f108pro_layout();
        assert_eq!(layout.id, "aula.f108pro.ansi");
        assert_eq!(layout.device_model_id, "aula.f108pro");
        assert_eq!(layout.keys.len(), 104);
        assert!(layout.keys.iter().all(|key| key.matrix.is_none()));
        assert!(layout.keys.iter().all(|key| key.grid_position.is_some()));
    }

    #[test]
    fn f108_geometry_and_indices_match_known_reference_keys() {
        let layout = f108pro_layout();

        let escape = layout
            .keys
            .iter()
            .find(|key| key.id == "key-001")
            .unwrap();
        assert_eq!(escape.label, "Esc");
        assert_eq!(escape.x, 27.0);
        assert_eq!(escape.y, 52.0);
        assert_eq!(escape.device_key_index, Some(1));
        assert_eq!(escape.led_index, Some(1));

        let space = layout
            .keys
            .iter()
            .find(|key| key.id == "key-094")
            .unwrap();
        assert_eq!(space.label, "Space");
        assert_eq!(space.width, 204.0);
        assert_eq!(space.device_key_index, Some(94));

        let numpad_enter = layout
            .keys
            .iter()
            .find(|key| key.id == "key-106")
            .unwrap();
        assert_eq!(numpad_enter.height, 59.0);
        assert_eq!(numpad_enter.led_index, Some(106));
    }
}
