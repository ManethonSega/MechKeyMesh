include!("lib.rs");

mod keymap;
mod macros;
mod per_key_rgb;

pub use keymap::{DesiredKeymap, FN_LAYER_ID, KeyMapping, NORMAL_LAYER_ID, layer_id};
pub use macros::{
    MAX_SOFTWARE_DELAY_MS, MAX_SOFTWARE_MACRO_EVENTS, MacroEvent, MacroMouseButton, MacroPlayback,
    MacroProgram, MacroSimulation, MacroValidationError, SimulatedMacroEvent,
    validate_macro_library,
};
pub use per_key_rgb::{DesiredPerKeyRgb, KeyColor};
