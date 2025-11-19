mod config;
mod error;
mod keybinds;
mod signals;
mod bar;

use config::Config;
use error::*;
use keybinds::*;
use signals::*;
use bar::*;

use log::*;
use std::{
    collections::BTreeSet, ffi::CString, mem::zeroed, process::Command, slice, sync::mpsc, thread,
    time::Duration,
};
use x11::{
    xinerama,
    xlib::{
        self, Cursor, GCForeground, XAllocColor, XButtonPressedEvent, XClearWindow, XColor,
        XCreateFontCursor, XDefaultColormap, XDefaultRootWindow, XDefaultScreen, XFlush, XGCValues,
        XParseColor, XSetWindowBackground, XUnmapWindow, XWindowAttributes,
    },
};

pub struct GridWM {
    display: *mut xlib::Display,
    config: Config,
    desktops: Vec<BTreeSet<Window>>,
    current_desktop: usize,
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
        let current_desktop = 0;

        let display: *mut xlib::Display =
            unsafe { xlib::XOpenDisplay(CString::new(display_name)?.as_ptr()) };

        if display.is_null() {
            return Err(GridWMError::DisplayNotFound(display_name.into()));
        }

        let desktops: Vec<BTreeSet<Window>> = Vec::new();

        // load config
        let config_path = dirs::config_dir()
            .map(|mut p| { p.push("gridwm/gridwm.toml"); p })
            .filter(|p| p.exists())
            .map(|p| p.to_str().unwrap_or("").to_string())
            .unwrap_or_else(|| {
            warn!("config file not found, using default");
            "".to_string()
            });
        let config = Config::from_file(&config_path)?;

        Ok(GridWM {
            display,
            config,
            desktops,
            current_desktop,
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
        if !self.config.keyboard.layout.is_empty() {
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
        } else {
            info!("not setting keyboard layout by user choice");
        }

        // default pointy cursor
        let cursor: Cursor = unsafe { XCreateFontCursor(self.display, 68) };

        // setting da mouse acceleationnnn vrooom
        let (accel_numerator, accel_denominator) =
            match self.config.mouse.acceleration_value.as_fraction() {
                Some((a, b)) => (a, b),
                None => {
                    warn!("failed to get mouse acceleration. falling back to default.");
                    (1, 1)
                }
            };

        // asdhu9aduiahidadhnihihasdhiahdoagfilkzurl
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
                .gridwm
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

            // draw bar for the first time yahooooo
            if self.config.bar.enable {
                self.draw_bar();
            }
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

        let (timer_tx, timer_rx) = mpsc::channel();

        thread::spawn(move || {
            loop {
                thread::sleep(Duration::from_secs(1));
                timer_tx.send(()).ok();
            }
        });

        let mut event: xlib::XEvent = unsafe { zeroed() };

        loop {
            while unsafe { xlib::XPending(self.display) } > 0 {
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
                            let desktop = self.get_desktop(self.current_desktop);
                            if desktop.contains(&map_event.window) {
                                xlib::XSetInputFocus(
                                    self.display,
                                    map_event.window,
                                    xlib::RevertToPointerRoot,
                                    xlib::CurrentTime,
                                );
                                xlib::XRaiseWindow(self.display, map_event.window);
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
                            // debug!("event triggered: {:?}", event);
                        }
                    }
                }
            }

            if timer_rx.try_recv().is_ok() && self.config.bar.enable {
                self.draw_bar();
            }
        }
    }

    fn set_desktop(&mut self, index: usize, value: BTreeSet<Window>) {
        if self.desktops.len() <= index {
            self.desktops.resize_with(index + 1, || BTreeSet::new());
        }
        self.desktops[index] = value;
    }

    fn get_desktop(&self, index: usize) -> BTreeSet<Window> {
        match self.desktops.get(index) {
            Some(d) => d.clone(),
            None => BTreeSet::new(),
        }
    }

    fn create_window(&mut self, event: xlib::XEvent) {
        info!("creating a window");
        let event: xlib::XMapRequestEvent = From::from(event);
        unsafe { xlib::XMapWindow(self.display, event.window) };
        let mut desktop = self.get_desktop(self.current_desktop);
        desktop.insert(event.window);
        self.set_desktop(self.current_desktop, desktop);
    }

    fn remove_window(&mut self, event: xlib::XEvent) {
        let event: xlib::XUnmapEvent = From::from(event);
        let mut desktop = self.get_desktop(self.current_desktop);
        desktop.remove(&event.window);
        self.set_desktop(self.current_desktop, desktop);
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

    fn handle_key(&mut self, event: xlib::XEvent) {
        let event: xlib::XKeyPressedEvent = From::from(event);

        // check keybindings and execute
        let gridwm_binds = self.config.keybinds.gridwm.clone();
        for bind in &gridwm_binds {
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
                    "desktop_right" => {
                        self.change_desktop(self.current_desktop + 1);
                    }
                    "desktop_left" => {
                        self.change_desktop(if self.current_desktop > 0 {
                            self.current_desktop - 1
                        } else {
                            0
                        });
                    }
                    _ => {}
                }
            }
        }

        // check keybindings for commands and execute
        let exec_binds = self.config.keybinds.exec.clone();
        for bind in &exec_binds {
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

    fn change_desktop(&mut self, index: usize) {
        if index == self.current_desktop {
            return;
        }
        unsafe {
            for window in self.get_desktop(self.current_desktop) {
                xlib::XUnmapWindow(self.display, window);
                xlib::XUnmapSubwindows(self.display, window);
            }
            for window in self.get_desktop(index) {
                xlib::XMapWindow(self.display, window);
                xlib::XMapSubwindows(self.display, window);
            }

            self.current_desktop = index;
        }
        
        if self.config.bar.enable {
            self.draw_bar();
        }
    }

    fn draw_bar(&self) {
        unsafe {
            let root = XDefaultRootWindow(self.display);

            let bar_str = match CString::new(format!("Desktop {} | {} | {}", self.current_desktop + 1, time_widget(), cpu_widget())) {
                Ok(stri) => stri,
                Err(e) => {
                    warn!(
                        "failed to create cstring for current desktop number: {}.",
                        e
                    );
                    return;
                }
            };

            let screen = XDefaultScreen(self.display);
            let mut bar_color: XColor = std::mem::zeroed();
            let mut background_color: XColor = std::mem::zeroed();

            let bar_hex_str = match CString::new(self.config.bar.text_color.clone()) {
                Ok(hex_str) => hex_str,
                Err(e) => {
                    error!("failed to convert bar text color str to cstring: {}", e);
                    return;
                }
            };

            let background_hex_str = match CString::new(self.config.bar.background_color.clone()) {
                Ok(hex_str) => hex_str,
                Err(e) => {
                    error!(
                        "failed to convert bar background color str to cstring: {}",
                        e
                    );
                    return;
                }
            };

            if XParseColor(
                self.display,
                XDefaultColormap(self.display, screen),
                bar_hex_str.as_ptr(),
                &mut bar_color,
            ) != 1
            {
                error!("failed to parse bar text color");
            }

            if XParseColor(
                self.display,
                XDefaultColormap(self.display, screen),
                background_hex_str.as_ptr(),
                &mut background_color,
            ) != 1
            {
                error!("failed to parse bar background color");
            }

            XAllocColor(
                self.display,
                XDefaultColormap(self.display, screen),
                &mut bar_color,
            );

            XAllocColor(
                self.display,
                XDefaultColormap(self.display, screen),
                &mut background_color,
            );

            let bar_gcv = XGCValues {
                foreground: bar_color.pixel,
                background: 0,
                font: 0,
                ..std::mem::zeroed()
            };

            let background_gcv = XGCValues {
                foreground: background_color.pixel,
                background: 0,
                font: 0,
                ..std::mem::zeroed()
            };

            let bar_gc = xlib::XCreateGC(
                self.display,
                root,
                GCForeground as u64,
                &bar_gcv as *const _ as *mut _,
            );

            let background_gc = xlib::XCreateGC(
                self.display,
                root,
                GCForeground as u64,
                &background_gcv as *const _ as *mut _,
            );

            if let Ok((screen_w, _)) = self.get_screen_size() {
                xlib::XClearArea(self.display, root, 0, 0, screen_w as u32, 50, 0);
                xlib::XFillRectangle(
                    self.display,
                    root,
                    background_gc,
                    0,
                    0,
                    screen_w as u32,
                    self.config.bar.height,
                );
            } else {
                xlib::XClearArea(self.display, root, 0, 0, 500, 50, 0);
            }
            xlib::XDrawString(
                self.display,
                root,
                bar_gc,
                5,
                15,
                bar_str.as_ptr() as *const i8,
                bar_str.to_bytes().len() as i32,
            );
        }
    }

    fn layout(&self) {
        let desktop = self.get_desktop(self.current_desktop);
        let tileable: Vec<Window> = desktop
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

        if self.config.bar.enable {
            self.draw_bar();
        }
    }

    fn tile(&self, n: usize, screen_w: i32, screen_h: i32) -> Vec<WindowInfo> {
        let cols = (n as f32).sqrt().ceil() as i32;
        let rows = ((n as i32 + cols - 1) / cols) as i32;
        let w = screen_w / cols;
        let mut h = screen_h / rows;
        if self.config.bar.enable {
            h = (screen_h
                - if screen_h < self.config.bar.height as i32 {
                    0
                } else {
                    self.config.bar.height as i32
                })
                / rows;
        }


        (0..n)
            .map(|i| {
                let i = i as i32;
                if self.config.bar.enable {
                    WindowInfo {
                        x: (i % cols) * w,
                        y: ((i / cols) * h) + self.config.bar.height as i32,
                        w,
                        h,
                    }
                } else {
                    WindowInfo {
                        x: (i % cols) * w,
                        y: (i / cols) * h,
                        w,
                        h,
                    }
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
            let dialog_type = xlib::XInternAtom(
                self.display,
                b"_NET_WM_WINDOW_TYPE_DIALOG\0".as_ptr() as *const i8,
                0,
            );
            let splash_type = xlib::XInternAtom(
                self.display,
                b"_NET_WM_WINDOW_TYPE_SPLASH\0".as_ptr() as *const i8,
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
                0,
                1,
                0,
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
                return wtype != notification_type
                    && wtype != dock_type
                    && wtype != dialog_type
                    && wtype != splash_type;
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
