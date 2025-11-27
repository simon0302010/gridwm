use num_rational::Ratio;
use serde::Deserialize;
use std::fs;

use crate::gridwm::error::GridWMError;

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub start: Start,
    pub general: General,
    pub keyboard: Keyboard,
    pub mouse: Mouse,
    pub desktop: Desktop,
    pub bar: Bar,
    pub keybinds: Keybinds,
}

// general section of config
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct General {
    pub update_ms: u64,
    pub scale_steps: u32,
}

impl Default for General {
    fn default() -> Self {
        Self {
            update_ms: 5,
            scale_steps: 20,
        }
    }
}

// keyboard section of config
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Keyboard {
    pub layout: String,
}

impl Default for Keyboard {
    fn default() -> Self {
        Self { layout: "".into() }
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
            color: "#464646".into(),
        }
    }
}

// bar section of config
#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct Bar {
    pub text_color: String,
    pub background_color: String,
    pub height: u32,
    pub enable: bool,
    pub update: f32,
    pub widgets: Vec<String>,
}

impl Default for Bar {
    fn default() -> Self {
        Self {
            text_color: "#ffffff".into(),
            background_color: "#000000".into(),
            height: 20,
            enable: true,
            update: 1.0,
            widgets: vec!["desktop".to_owned()],
        }
    }
}

// keybinds section of config
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Keybinds {
    pub gridwm: Vec<Vec<String>>,
    pub exec: Vec<Vec<String>>,
    pub move_mod: String,
    pub resize_mod: String,
}

impl Default for Keybinds {
    fn default() -> Self {
        Self {
            gridwm: Vec::new(),
            exec: Vec::new(),
            move_mod: "SUPER".to_string(),
            resize_mod: "SUPER".to_string(),
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
        match fs::read_to_string(path) {
            Ok(s) => {
                let cfg: Config = toml::from_str(&s)?;
                Ok(cfg)
            }
            Err(_) => Ok(Config::default()),
        }
    }
}
