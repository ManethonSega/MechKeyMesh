use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const MAX_SOFTWARE_MACRO_EVENTS: usize = 10_000;
pub const MAX_SOFTWARE_DELAY_MS: u32 = 3_600_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MacroMouseButton {
    Left,
    Right,
    Middle,
    Button4,
    Button5,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum MacroEvent {
    KeyDown { usage: u16 },
    KeyUp { usage: u16 },
    MouseDown { button: MacroMouseButton },
    MouseUp { button: MacroMouseButton },
    Delay { milliseconds: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "kebab-case")]
pub enum MacroPlayback {
    Once,
    Repeat { count: u16 },
    RepeatWhileHeld,
    Toggle,
    ReleaseToStop,
}

impl Default for MacroPlayback {
    fn default() -> Self {
        Self::Once
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MacroProgram {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub playback: MacroPlayback,
    #[serde(default)]
    pub events: Vec<MacroEvent>,
}

impl MacroProgram {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            playback: MacroPlayback::Once,
            events: Vec::new(),
        }
    }

    pub fn validate(&self) -> Result<(), MacroValidationError> {
        require_text(&self.id, "macro ID", 96)?;
        require_text(&self.name, "macro name", 160)?;
        if self.events.len() > MAX_SOFTWARE_MACRO_EVENTS {
            return Err(MacroValidationError::TooManyEvents {
                count: self.events.len(),
                maximum: MAX_SOFTWARE_MACRO_EVENTS,
            });
        }
        if let MacroPlayback::Repeat { count } = self.playback
            && count == 0
        {
            return Err(MacroValidationError::InvalidPlayback(
                "repeat count must be at least 1".into(),
            ));
        }

        for (index, event) in self.events.iter().enumerate() {
            match event {
                MacroEvent::KeyDown { usage } | MacroEvent::KeyUp { usage }
                    if *usage == 0 =>
                {
                    return Err(MacroValidationError::InvalidEvent {
                        index,
                        reason: "keyboard usage 0 is reserved".into(),
                    });
                }
                MacroEvent::Delay { milliseconds }
                    if *milliseconds > MAX_SOFTWARE_DELAY_MS =>
                {
                    return Err(MacroValidationError::InvalidEvent {
                        index,
                        reason: format!(
                            "delay {milliseconds} ms exceeds the software limit of {MAX_SOFTWARE_DELAY_MS} ms"
                        ),
                    });
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub fn simulation(&self) -> Result<MacroSimulation, MacroValidationError> {
        self.validate()?;
        let mut elapsed_milliseconds = 0u64;
        let mut steps = Vec::new();
        for (event_index, event) in self.events.iter().cloned().enumerate() {
            if let MacroEvent::Delay { milliseconds } = event {
                elapsed_milliseconds = elapsed_milliseconds
                    .checked_add(u64::from(milliseconds))
                    .ok_or(MacroValidationError::DurationOverflow)?;
            } else {
                steps.push(SimulatedMacroEvent {
                    event_index,
                    elapsed_milliseconds,
                    event,
                });
            }
        }
        Ok(MacroSimulation {
            playback: self.playback.clone(),
            duration_milliseconds: elapsed_milliseconds,
            steps,
            hardware_touched: false,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SimulatedMacroEvent {
    pub event_index: usize,
    pub elapsed_milliseconds: u64,
    pub event: MacroEvent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MacroSimulation {
    pub playback: MacroPlayback,
    pub duration_milliseconds: u64,
    pub steps: Vec<SimulatedMacroEvent>,
    pub hardware_touched: bool,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum MacroValidationError {
    #[error("{field} must not be empty")]
    EmptyText { field: &'static str },
    #[error("{field} exceeds {maximum} characters")]
    TextTooLong {
        field: &'static str,
        maximum: usize,
    },
    #[error("macro contains {count} events, maximum is {maximum}")]
    TooManyEvents { count: usize, maximum: usize },
    #[error("invalid playback: {0}")]
    InvalidPlayback(String),
    #[error("invalid event {index}: {reason}")]
    InvalidEvent { index: usize, reason: String },
    #[error("macro duration overflow")]
    DurationOverflow,
    #[error("duplicate macro ID: {0}")]
    DuplicateId(String),
}

pub fn validate_macro_library(macros: &[MacroProgram]) -> Result<(), MacroValidationError> {
    let mut ids = HashSet::new();
    for program in macros {
        program.validate()?;
        if !ids.insert(program.id.as_str()) {
            return Err(MacroValidationError::DuplicateId(program.id.clone()));
        }
    }
    Ok(())
}

fn require_text(
    value: &str,
    field: &'static str,
    maximum: usize,
) -> Result<(), MacroValidationError> {
    if value.trim().is_empty() {
        return Err(MacroValidationError::EmptyText { field });
    }
    if value.chars().count() > maximum {
        return Err(MacroValidationError::TextTooLong { field, maximum });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeline_simulation_accumulates_explicit_delays_without_hardware() {
        let program = MacroProgram {
            id: "movement-copy".into(),
            name: "Movement Copy".into(),
            playback: MacroPlayback::Once,
            events: vec![
                MacroEvent::KeyDown { usage: 0x1a },
                MacroEvent::Delay { milliseconds: 83 },
                MacroEvent::KeyUp { usage: 0x1a },
                MacroEvent::Delay { milliseconds: 27 },
                MacroEvent::MouseDown {
                    button: MacroMouseButton::Left,
                },
            ],
        };

        let simulation = program.simulation().unwrap();
        assert_eq!(simulation.duration_milliseconds, 110);
        assert_eq!(simulation.steps[0].elapsed_milliseconds, 0);
        assert_eq!(simulation.steps[1].elapsed_milliseconds, 83);
        assert_eq!(simulation.steps[2].elapsed_milliseconds, 110);
        assert!(!simulation.hardware_touched);
    }

    #[test]
    fn all_f108_playback_concepts_are_representable() {
        for playback in [
            MacroPlayback::Once,
            MacroPlayback::Repeat { count: 3 },
            MacroPlayback::RepeatWhileHeld,
            MacroPlayback::Toggle,
            MacroPlayback::ReleaseToStop,
        ] {
            let mut program = MacroProgram::new("test", "Test");
            program.playback = playback;
            program.events.push(MacroEvent::KeyDown { usage: 4 });
            assert!(program.validate().is_ok());
        }
    }

    #[test]
    fn invalid_delays_usages_and_repeat_counts_are_rejected() {
        let mut program = MacroProgram::new("test", "Test");
        program.events.push(MacroEvent::KeyDown { usage: 0 });
        assert!(program.validate().is_err());

        program.events = vec![MacroEvent::Delay {
            milliseconds: MAX_SOFTWARE_DELAY_MS + 1,
        }];
        assert!(program.validate().is_err());

        program.events.clear();
        program.playback = MacroPlayback::Repeat { count: 0 };
        assert!(program.validate().is_err());
    }
}
