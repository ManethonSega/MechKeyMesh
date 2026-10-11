include!("lib.rs");

mod keymap;
mod per_key_rgb;

pub use keymap::{DesiredKeymap, FN_LAYER_ID, KeyMapping, NORMAL_LAYER_ID, layer_id};
pub use per_key_rgb::{DesiredPerKeyRgb, KeyColor};
