use log::*;
use std::{
    ffi::{CString, NulError},
    mem::zeroed,
};
use x11::xlib;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum GridWMError {
    #[error("display {0} not found")]
    DisplayNotFound(String),

    #[error("{0}")]
    NulString(#[from] NulError),
}

pub struct GridWM {
    display: *mut xlib::Display,
}

impl GridWM {
    pub fn new(display_name: &str) -> Result<Self, GridWMError> {
        match simple_logger::init() {
            Ok(_) => {}
            Err(e) => {
                println!("Failed to start logger: {}", e);
            }
        }

        let display: *mut xlib::Display =
            unsafe { xlib::XOpenDisplay(CString::new(display_name)?.as_ptr()) };

        if display.is_null() {
            return Err(GridWMError::DisplayNotFound(display_name.into()));
        }

        Ok(GridWM { display })
    }

    pub fn init(&self) -> Result<(), GridWMError> {
        unsafe {
            xlib::XSelectInput(
                self.display,
                xlib::XDefaultRootWindow(self.display),
                xlib::SubstructureRedirectMask,
            );
        }
        Ok(())
    }

    pub fn run(&self) {
        info!("gridwm running");

        let mut event: xlib::XEvent = unsafe { zeroed() };
        loop {
            unsafe {
                xlib::XNextEvent(self.display, &mut event);

                match event.get_type() {
                    xlib::MapRequest => {
                        self.create_window(event);
                    }
                    _ => {
                        warn!("unknown event: {:?}", event);
                    }
                }
            }
        }
    }

    fn create_window(&self, event: xlib::XEvent) {
        info!("creating a window");
        let event: xlib::XMapRequestEvent = From::from(event);
        unsafe { xlib::XMapWindow(self.display, event.window) };
    }
}
