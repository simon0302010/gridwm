mod config;
mod error;
mod keybinds;
mod signals;

use config::Config;
use error::*;
use keybinds::*;
use signals::*;

use log::*;
use std::{
    collections::BTreeSet, ffi::CString, mem::zeroed, os::unix::process::CommandExt,
    process::Command, slice,
};
use x11::{
    xinerama,
    xlib::{
        self, Cursor, XAllocColor, XButtonPressedEvent, XClearWindow, XColor, XCreateFontCursor,
        XDefaultColormap, XDefaultRootWindow, XDefaultScreen, XFlush, XParseColor,
        XSetWindowBackground, XUnmapWindow, XWindowAttributes,
    },
};

pub struct GridWM {
    display: *mut xlib::Display,
    windows: BTreeSet<Window>,
    config: Config,
}

pub type Window = u64;

struct WindowInfo {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

impl GridWM {
    pub fn new(display_name: &str) -> Result<Self, GridWMError> {
        let display: *mut xlib::Display =
            unsafe { xlib::XOpenDisplay(CString::new(display_name)?.as_ptr()) };

        if display.is_null() {
            return Err(GridWMError::DisplayNotFound(display_name.into()));
        }

        // create set to store windows
        let windows: BTreeSet<u64> = BTreeSet::new();

        // load config
        let config = Config::from_file("gridwm.toml")?; // load config here

        Ok(GridWM {
            display,
            windows,
            config,
        })
    }

    pub fn init(&self) -> Result<(), GridWMError> {
        match simple_logger::init() {
            Ok(_) => {}
            Err(e) => {
                println!("Failed to start logger: {}", e);
            }
        }

        // set keyboard layout
        match Command::new("setxkbmap")
            .arg(self.config.keyboard.layout.clone())
            .spawn()
            .and_then(|mut child| child.wait())
        {
            Ok(_) => {}
            Err(e) => {
                error!("failed to set keyboard layout: {}", e);
            }
        }

        let cursor: Cursor = unsafe { XCreateFontCursor(self.display, 68) };

        let (accel_numerator, accel_denominator) =
            match self.config.mouse.acceleration_value.as_fraction() {
                Some((a, b)) => (a, b),
                None => {
                    warn!("failed to get mouse acceleration. falling back to default.");
                    (1, 1)
                }
            };

        unsafe {
            let root = XDefaultRootWindow(self.display);

            xlib::XSelectInput(
                self.display,
                root,
                xlib::SubstructureRedirectMask | xlib::SubstructureNotifyMask,
            );

            const EXTRA_MODS: [u32; 4] = [
                0,
                xlib::LockMask,
                xlib::Mod2Mask,
                xlib::LockMask | xlib::Mod2Mask,
            ];

            // grab keys for keybindings
            for bind in self
                .config
                .keybinds
                .window
                .iter()
                .chain(self.config.keybinds.exec.iter())
            {
                if bind.len() != 2 {
                    error!("failed to parse keybind {:?}: invalid length.", bind);
                    continue;
                }

                let (mask, keycode): (u32, i32) = match parse_keybind(self.display, bind[0].clone())
                {
                    Some((a, b)) => (a, b),
                    None => {
                        warn!("failed to parse keybind: {:?}", bind);
                        continue;
                    }
                };

                for &extra_mod in &EXTRA_MODS {
                    xlib::XGrabKey(
                        self.display,
                        keycode,
                        mask | extra_mod,
                        root,
                        0,
                        xlib::GrabModeAsync,
                        xlib::GrabModeAsync,
                    );
                }
            }

            xlib::XGrabButton(
                self.display,
                xlib::Button1,
                xlib::AnyModifier,
                root,
                1,
                (xlib::ButtonPressMask | xlib::ButtonReleaseMask) as u32,
                xlib::GrabModeSync,
                xlib::GrabModeAsync,
                0,
                0,
            );

            xlib::XChangePointerControl(
                self.display,
                self.config.mouse.use_acceleration as i32,
                self.config.mouse.use_acceleration_threshold as i32,
                accel_numerator,
                accel_denominator,
                self.config.mouse.acceleration_threshold,
            );

            xlib::XDefineCursor(self.display, root, cursor);

            // set background yay
            self.set_background(self.config.desktop.color.clone());

            // flush the toilet
            XFlush(self.display);
        }
        Ok(())
    }

    pub fn run(&mut self) {
        info!("gridwm running");

        for start_job in &self.config.start.exec {
            match shell_words::split(start_job) {
                Ok(parts) => {
                    let program = &parts[0];
                    let args = &parts[1..];
                    let _ = Command::new(program).args(args).spawn();
                }
                Err(e) => {
                    error!("Failed to parse start job '{}': {}", start_job, e);
                }
            }
        }

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
                        self.layout();
                    }
                    xlib::MapNotify => {
                        // set focus when window is mapped
                        let map_event: xlib::XMapEvent = From::from(event);
                        if self.windows.contains(&map_event.window) {
                            xlib::XSetInputFocus(
                                self.display,
                                map_event.window,
                                xlib::RevertToPointerRoot,
                                xlib::CurrentTime,
                            );
                        }
                        self.layout();
                    }
                    xlib::KeyPress => {
                        self.handle_key(event);
                    }
                    xlib::ButtonPress => {
                        self.handle_button(event);
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
            let screen = screens.first();

            if let Some(screen) = screen {
                Ok((screen.width, screen.height))
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

    fn _get_window_attributes(&self, window: Window) -> XWindowAttributes {
        let mut attrs: XWindowAttributes = unsafe { zeroed() };
        unsafe {
            xlib::XGetWindowAttributes(self.display, window, &mut attrs);
        }
        attrs
    }

    fn handle_key(&self, event: xlib::XEvent) {
        let event: xlib::XKeyPressedEvent = From::from(event);

        // check keybindings and execute
        for bind in &self.config.keybinds.window {
            if bind.len() != 2 {
                error!("failed to parse keybind {:?}: invalid length.", bind);
                continue;
            }

            let (mask, keycode): (u32, i32) = match parse_keybind(self.display, bind[0].clone()) {
                Some((a, b)) => (a, b),
                None => {
                    warn!("failed to parse keybind: {:?}", bind);
                    continue;
                }
            };

            let relevant_modifiers: u32 =
                xlib::ControlMask | xlib::ShiftMask | xlib::Mod1Mask | xlib::Mod4Mask;
            let event_mask = event.state & relevant_modifiers;

            if event_mask == mask && event.keycode as i32 == keycode {
                match bind[1].as_str() {
                    "close" => {
                        if event.subwindow != unsafe { XDefaultRootWindow(self.display) }
                            && event.subwindow != 0
                        {
                            send_wm_delete_window(self.display, event.subwindow);
                            unsafe {
                                XUnmapWindow(self.display, event.subwindow);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        // check keybindings for commands and execute
        for bind in &self.config.keybinds.exec {
            if bind.len() != 2 {
                error!("failed to parse keybind {:?}: invalid length.", bind);
                continue;
            }

            let (mask, keycode): (u32, i32) = match parse_keybind(self.display, bind[0].clone()) {
                Some((a, b)) => (a, b),
                None => {
                    warn!("failed to parse keybind: {:?}", bind);
                    continue;
                }
            };

            let relevant_modifiers: u32 =
                xlib::ControlMask | xlib::ShiftMask | xlib::Mod1Mask | xlib::Mod4Mask;
            let event_mask = event.state & relevant_modifiers;

            if event_mask == mask && event.keycode as i32 == keycode {
                match shell_words::split(&bind[1].as_str()) {
                    Ok(parts) => {
                        let program = &parts[0];
                        let args = &parts[1..];
                        let _ = Command::new(program).args(args).spawn();
                    }
                    Err(e) => {
                        error!("Failed to parse keybinding '{}': {}", bind[1], e);
                    }
                }
            }
        }
    }

    fn handle_button(&self, event: xlib::XEvent) {
        let event: XButtonPressedEvent = From::from(event);

        if event.subwindow != 0 {
            unsafe {
                xlib::XSetInputFocus(
                    self.display,
                    event.subwindow,
                    xlib::RevertToPointerRoot,
                    xlib::CurrentTime,
                );
                xlib::XRaiseWindow(self.display, event.subwindow);
                XFlush(self.display);
            }
        }
        unsafe {
            xlib::XAllowEvents(self.display, xlib::ReplayPointer, xlib::CurrentTime);
            XFlush(self.display);
        }
    }

    fn layout(&self) {
        let tileable: Vec<Window> = self.windows
            .iter()
            .copied()
            .filter(|&w| self.is_tileable(w))
            .collect();

        if tileable.is_empty() {
            return;
        }

        if let Ok((screen_w, screen_h)) = self.get_screen_size() {
            let positions = self.tile(tileable.len(), screen_w as i32, screen_h as i32);
            for (id, window) in tileable.iter().zip(positions) {
                self.resize_window(*id, window.w as u32, window.h as u32);
                self.move_window(*id, window.x, window.y);
            }
        }
    }

    fn tile(&self, n: usize, screen_w: i32, screen_h: i32) -> Vec<WindowInfo> {
        let cols = (n as f32).sqrt().ceil() as i32;
        let rows = ((n as i32 + cols - 1) / cols) as i32;
        let w = screen_w / cols;
        let h = screen_h / rows;

        (0..n)
            .map(|i| {
                let i = i as i32;
                WindowInfo {
                    x: (i % cols) * w,
                    y: (i / cols) * h,
                    w,
                    h,
                }
            })
            .collect()
    }

    fn is_tileable(&self, window: Window) -> bool {
        unsafe {
            let window_type = xlib::XInternAtom(
                self.display,
                b"_NET_WM_WINDOW_TYPE\0".as_ptr() as *const i8,
                0,
            );
            let notification_type = xlib::XInternAtom(
                self.display,
                b"_NET_WM_WINDOW_TYPE_NOTIFICATION\0".as_ptr() as *const i8,
                0,
            );
            let dock_type = xlib::XInternAtom(
                self.display,
                b"_NET_WM_WINDOW_TYPE_DOCK\0".as_ptr() as *const i8,
                0,
            );

            let mut actual_type: xlib::Atom = 0;
            let mut actual_format: i32 = 0;
            let mut nitems: u64 = 0;
            let mut bytes_after: u64 = 0;
            let mut prop: *mut u8 = std::ptr::null_mut();

            if xlib::XGetWindowProperty(
                self.display, 
                window, 
                window_type, 
                0, 1, 0, 
                xlib::XA_ATOM, 
                &mut actual_type, 
                &mut actual_format, 
                &mut nitems, 
                &mut bytes_after,
                &mut prop,
            ) == 0
                && nitems > 0
            {
                let wtype = *(prop as *const xlib::Atom);
                libc::free(prop as *mut _);
                return wtype != notification_type && wtype != dock_type;
            }
            true
        }
    }

    fn set_background(&self, hex: String) {
        let root_window = unsafe { XDefaultRootWindow(self.display) };
        unsafe {
            let screen = XDefaultScreen(self.display);

            let mut color: XColor = std::mem::zeroed();

            let hex_str = match CString::new(hex) {
                Ok(hex_str) => hex_str,
                Err(e) => {
                    error!("failed to convert background color str to cstring: {}", e);
                    return;
                }
            };

            if XParseColor(
                self.display,
                XDefaultColormap(self.display, screen),
                hex_str.as_ptr(),
                &mut color,
            ) != 1
            {
                error!("failed to parse background color");
            }

            XAllocColor(
                self.display,
                XDefaultColormap(self.display, screen),
                &mut color,
            );

            XSetWindowBackground(self.display, root_window, color.pixel);

            XClearWindow(self.display, root_window);
        };
    }
}
