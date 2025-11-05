use std::{error::Error, ffi::NulError, fmt};

#[derive(Debug)]
pub enum GridWMError {
    DisplayNotFound(String),
    NulString(NulError),
    ScreenNotFound(String),
    ConfigLoadFailed(String),
}

impl fmt::Display for GridWMError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GridWMError::ConfigLoadFailed(config) => write!(f, "failed to load config: {}", config),
            GridWMError::DisplayNotFound(display) => write!(f, "display {} not found", display),
            GridWMError::NulString(e) => write!(f, "{}", e),
            GridWMError::ScreenNotFound(screen) => write!(f, "screen {} not found", screen),
        }
    }
}

impl Error for GridWMError {}

impl From<NulError> for GridWMError {
    fn from(err: NulError) -> GridWMError {
        GridWMError::NulString(err)
    }
}
