use std::{error::Error, ffi::NulError, fmt};

#[derive(Debug)]
pub enum GridWMError {
    Io(std::io::Error),
    Toml(toml::de::Error),
    DisplayNotFound(String),
    NulString(NulError),
    ScreenNotFound(String),
}

impl fmt::Display for GridWMError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GridWMError::DisplayNotFound(display) => write!(f, "display {} not found", display),
            GridWMError::NulString(e) => write!(f, "{}", e),
            GridWMError::ScreenNotFound(screen) => write!(f, "screen {} not found", screen),
            GridWMError::Io(err) => write!(f, "io error: {}", err),
            GridWMError::Toml(err) => write!(f, "failed to parse toml: {}", err),
        }
    }
}

impl Error for GridWMError {}

impl From<NulError> for GridWMError {
    fn from(err: NulError) -> GridWMError {
        GridWMError::NulString(err)
    }
}

impl From<std::io::Error> for GridWMError {
    fn from(err: std::io::Error) -> GridWMError {
        GridWMError::Io(err)
    }
}

impl From<toml::de::Error> for GridWMError {
    fn from(err: toml::de::Error) -> GridWMError {
        GridWMError::Toml(err)
    }
}
