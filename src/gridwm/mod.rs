use log::*;
use std::{
    collections::BTreeSet,
    ffi::{CString, NulError},
    mem::zeroed,
    slice,
};
use x11::{
    xinerama,
    xlib::{self, XWindowAttributes},
};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum GridWMError {
    #[error("display {0} not found")]
    DisplayNotFound(String),

    #[error("{0}")]
    NulString(#[from] NulError),

    #[error("screen {0} not found")]
    ScreenNotFound(String),
}

pub struct GridWM {
    display: *mut xlib::Display,
    windows: BTreeSet<Window>,
}

pub type Window = u64;

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

        let windows: BTreeSet<u64> = BTreeSet::new();

        Ok(GridWM { display, windows })
    }

    pub fn init(&self) -> Result<(), GridWMError> {
        unsafe {
            xlib::XSelectInput(
                self.display,
                xlib::XDefaultRootWindow(self.display),
                xlib::SubstructureRedirectMask | xlib::SubstructureNotifyMask,
            );
        }
        Ok(())
    }

    pub fn run(&mut self) {
        info!("gridwm running");

        let mut event: xlib::XEvent = unsafe { zeroed() };
        loop {
            unsafe {
                xlib::XNextEvent(self.display, &mut event);

                match event.get_type() {
                    xlib::MapRequest => {
                        self.create_window(event);
                    }
                    xlib::UnmapNotify => {
                        self.remove_window(event);
                    }
                    _ => {
                        debug!("event triggered: {:?}", event);
                    }
                }
            }
        }
    }

    fn create_window(&mut self, event: xlib::XEvent) {
        info!("creating a window");
        let event: xlib::XMapRequestEvent = From::from(event);
        unsafe { xlib::XMapWindow(self.display, event.window) };
        self.windows.insert(event.window);
    }

    fn remove_window(&mut self, event: xlib::XEvent) {
        let event: xlib::XUnmapEvent = From::from(event);
        match self.windows.remove(&event.window) {
            true => {
                info!("closed a window");
            }
            false => {
                warn!("tried removing not existing window")
            }
        }
    }

    fn get_screen_size(&self) -> Result<(i16, i16), GridWMError> {
        unsafe {
            let mut num: i32 = 0;
            let screen_pointers = xinerama::XineramaQueryScreens(self.display, &mut num);
            let screens = slice::from_raw_parts(screen_pointers, num as usize).to_vec();
            let screen = screens.get(0);

            if let Some(screen) = screen {
                Ok((screen.width, screen.width))
            } else {
                Err(GridWMError::ScreenNotFound("0".to_string()))
            }
        }
    }

    fn move_window(&self, window: Window, x: i32, y: i32) {
        unsafe { xlib::XMoveWindow(self.display, window, x, y) };
    }

    fn resize_window(&self, window: Window, width: u32, height: u32) {
        unsafe { xlib::XResizeWindow(self.display, window, width, height) };
    }

    fn get_window_attributes(&self, window: Window) -> XWindowAttributes {
        let mut attrs: XWindowAttributes = unsafe { zeroed() };
        unsafe {
            xlib::XGetWindowAttributes(self.display, window, &mut attrs);
        }
        attrs
    }
}
