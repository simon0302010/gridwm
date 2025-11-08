use num_rational::Ratio;
use serde::Deserialize;
use std::fs;

use crate::gridwm::error::GridWMError;

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub start: Start,
    pub keyboard: Keyboard,
    pub mouse: Mouse,
    pub desktop: Desktop,
    pub keybinds: Keybinds,
}

// keyboard section of config
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Keyboard {
    pub layout: String,
}

impl Default for Keyboard {
    fn default() -> Self {
        Self {
            layout: "us".into(),
        }
    }
}

// mouse section of config
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Mouse {
    pub use_acceleration: bool,
    pub use_acceleration_threshold: bool,
    pub acceleration_value: RatioF64,
    pub acceleration_threshold: i32,
}

impl Default for Mouse {
    fn default() -> Self {
        Self {
            use_acceleration: false,
            use_acceleration_threshold: false,
            acceleration_value: RatioF64(1.0),
            acceleration_threshold: 0,
        }
    }
}

// desktop section of config
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Desktop {
    pub color: String,
}

impl Default for Desktop {
    fn default() -> Self {
        Self {
            color: "#464646".to_string(),
        }
    }
}

// keybinds section of config
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Keybinds {
    pub window: Vec<Vec<String>>,
    pub exec: Vec<Vec<String>>,
}

impl Default for Keybinds {
    fn default() -> Self {
        Self {
            window: Vec::new(),
            exec: Vec::new(),
        }
    }
}

// start section of config
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct Start {
    pub exec: Vec<String>,
}

// ratio for mouse acceleration
#[derive(Debug, Copy, Clone, Deserialize)]
pub struct RatioF64(pub f64);

impl RatioF64 {
    pub fn as_fraction(&self) -> Option<(i32, i32)> {
        Ratio::approximate_float(self.0).map(|r| (*r.numer(), *r.denom()))
    }
}

// functions for config
impl Config {
    pub fn from_file(path: &str) -> Result<Self, GridWMError> {
        let s = fs::read_to_string(path)?;
        let cfg: Config = toml::from_str(&s)?;
        Ok(cfg)
    }
}
