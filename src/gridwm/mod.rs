mod bar;
mod config;
mod error;
mod keybinds;
mod signals;

use bar::*;
use config::Config;
use error::*;
use keybinds::*;
use signals::*;

use log::*;
use std::{
    collections::BTreeSet,
    ffi::CString,
    mem::zeroed,
    process::Command,
    slice,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
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
    drag_state: Option<DragState>,
    floating_windows: BTreeSet<Window>,
    bar_gc: xlib::GC,
    background_gc: xlib::GC,
    bar_str: String,
    screen_width: i16,
    screen_height: i16,
}

pub type Window = u64;

struct WindowInfo {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

#[derive(Debug, Clone, Copy)]
struct DragState {
    window: Window,
    start_win_x: i32,
    start_win_y: i32,
    start_mouse_x: i32,
    start_mouse_y: i32,
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
            .map(|mut p| {
                p.push("gridwm/gridwm.toml");
                p
            })
            .filter(|p| p.exists())
            .map(|p| p.to_str().unwrap_or("").to_string())
            .unwrap_or_else(|| {
                warn!("config file not found, using default");
                "".to_string()
            });
        let config = Config::from_file(&config_path)?;

        let (background_gc, bar_gc) = match create_bar_gc(display, &config) {
            Some(dat) => dat,
            None => {
                error!("failed to create bar and background gc. exiting.");
                std::process::exit(1);
            }
        };

        let mut bar_str = get_widgets(&config.bar.widgets);
        bar_str = bar_str.replace("DESKTOP_HERE", &desktop_widget(current_desktop));

        let (screen_width, screen_height) = match get_screen_size(display) {
            Ok(wh) => wh,
            Err(_) => {
                error!("failed to get screen size");
                return Err(GridWMError::ScreenNotFound("0".to_string()));
            }
        };

        Ok(GridWM {
            display,
            config,
            desktops,
            current_desktop,
            drag_state: None,
            floating_windows: BTreeSet::new(),
            background_gc,
            bar_gc,
            bar_str,
            screen_width,
            screen_height,
        })
    }

    pub fn init(&self) -> Result<(), GridWMError> {
        match simple_logger::init() {
            Ok(_) => {}
            Err(e) => {
                println!("failed to start logger: {}", e);
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
                    Some((a, Some(b))) => (a, b),
                    Some((_, None)) => {
                        continue;
                    }
                    _ => {
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

            // for dragging
            if let Some(modifier) = parse_modifier(&self.config.keybinds.move_mod) {
                for &extra_mod in &EXTRA_MODS {
                    xlib::XGrabButton(
                        self.display,
                        xlib::Button1,
                        modifier | extra_mod,
                        root,
                        1,
                        (xlib::ButtonPressMask | xlib::ButtonReleaseMask | xlib::Button1MotionMask)
                            as u32,
                        xlib::GrabModeAsync,
                        xlib::GrabModeAsync,
                        0,
                        0,
                    );
                }
            }

            // for resizing
            if let Some(modifier) = parse_modifier(&self.config.keybinds.resize_mod) {
                for &extra_mod in &EXTRA_MODS {
                    // scroll up
                    xlib::XGrabButton(
                        self.display,
                        xlib::Button4,
                        modifier | extra_mod,
                        root,
                        1,
                        (xlib::ButtonPressMask | xlib::ButtonReleaseMask | xlib::Button1MotionMask)
                            as u32,
                        xlib::GrabModeAsync,
                        xlib::GrabModeAsync,
                        0,
                        0,
                    );
                    // scroll down
                    xlib::XGrabButton(
                        self.display,
                        xlib::Button5,
                        modifier | extra_mod,
                        root,
                        1,
                        (xlib::ButtonPressMask | xlib::ButtonReleaseMask | xlib::Button1MotionMask)
                            as u32,
                        xlib::GrabModeAsync,
                        xlib::GrabModeAsync,
                        0,
                        0,
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
                self.draw_bar(None);
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
                    error!("failed to parse start job '{}': {}", start_job, e);
                }
            }
        }

        let (timer_tx, timer_rx) = mpsc::channel();

        if self.config.bar.enable {
            let bar_config = self.config.bar.clone();

            thread::spawn(move || {
                loop {
                    let proc_start = Instant::now();
                    let data = get_widgets(&bar_config.widgets);

                    if timer_tx.send(data).is_err() {
                        break;
                    }
                    let elapsed = proc_start.elapsed().as_millis();
                    if elapsed < (bar_config.update * 1000.0) as u128 {
                        thread::sleep(
                            Duration::from_millis((bar_config.update * 1000.0) as u64)
                                - proc_start.elapsed(),
                        );
                    } else {
                        warn!(
                            "bar update took {}ms, exceeds configured interval of {}ms",
                            elapsed,
                            (bar_config.update * 1000.0) as u128
                        )
                    }
                }
            });
        }

        let mut event: xlib::XEvent = unsafe { zeroed() };
        let mut too_long_count = 0;

        loop {
            let process_start = Instant::now();
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
                            let btn_event: xlib::XButtonPressedEvent = From::from(event);

                            let is_drag_bind = if let Some(mask) =
                                parse_modifier(&self.config.keybinds.move_mod)
                            {
                                // TODO: don't hardcode
                                let config_btn = xlib::Button1;
                                (btn_event.state & mask == mask) && (btn_event.button == config_btn)
                            } else {
                                false
                            };

                            let is_scroll_up = if let Some(mask) =
                                parse_modifier(&self.config.keybinds.resize_mod)
                            {
                                // TODO: don't hardcode
                                let config_btn = xlib::Button4;
                                (btn_event.state & mask == mask) && (btn_event.button == config_btn)
                            } else {
                                false
                            };

                            let is_scroll_down = if let Some(mask) =
                                parse_modifier(&self.config.keybinds.resize_mod)
                            {
                                // TODO: don't hardcode
                                let config_btn = xlib::Button5;
                                (btn_event.state & mask == mask) && (btn_event.button == config_btn)
                            } else {
                                false
                            };

                            // TODO: make resize direction configurable
                            if is_drag_bind {
                                self.handle_drag_start(btn_event);
                            } else if is_scroll_up {
                                self.scale_up(btn_event);
                            } else if is_scroll_down {
                                self.scale_down(btn_event);
                            } else {
                                self.handle_button(event);
                            }
                        }
                        xlib::MotionNotify => {
                            while xlib::XCheckTypedEvent(
                                self.display,
                                xlib::MotionNotify,
                                &mut event,
                            ) > 0
                            {}

                            let motion_event: xlib::XMotionEvent = From::from(event);

                            self.handle_motion(motion_event);
                        }
                        xlib::ButtonRelease => {
                            self.handle_release(From::from(event));
                        }
                        _ => {
                            // debug!("event triggered: {:?}", event);
                        }
                    }
                }
            }

            if let Ok(data) = timer_rx.try_recv()
                && self.config.bar.enable
            {
                self.bar_str = data;
            }

            if self.config.bar.enable {
                self.draw_bar(None);
            }

            // subtract time process took from sleep time
            let sleep_time = Duration::from_millis(self.config.general.update_ms);
            let process_took = process_start.elapsed();
            if process_took < sleep_time {
                too_long_count = 0;
                thread::sleep(sleep_time - process_took);
            } else {
                too_long_count += 1;
            }

            // it can sometimes take too long when resizing windows fast
            if too_long_count > 5 {
                warn!("main loop exceeded the expected duration 5 consecutive times");
            }
        }
    }

    fn set_desktop(&mut self, index: usize, value: BTreeSet<Window>) {
        if self.desktops.len() <= index {
            self.desktops.resize_with(index + 1, BTreeSet::new);
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
        self.floating_windows.remove(&event.window);
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
                Some((a, Some(b))) => (a, b),
                _ => {
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
                    "unfloat" => {
                        // unfloats all windows
                        self.floating_windows.clear();
                        self.layout();
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
                Some((a, Some(b))) => (a, b),
                _ => {
                    warn!("failed to parse keybind: {:?}", bind);
                    continue;
                }
            };

            let relevant_modifiers: u32 =
                xlib::ControlMask | xlib::ShiftMask | xlib::Mod1Mask | xlib::Mod4Mask;
            let event_mask = event.state & relevant_modifiers;

            if event_mask == mask && event.keycode as i32 == keycode {
                match shell_words::split(bind[1].as_str()) {
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

    fn get_toplevel(&self, mut window: Window) -> Window {
        unsafe {
            loop {
                let mut root: xlib::Window = 0;
                let mut parent: xlib::Window = 0;
                let mut children: *mut xlib::Window = std::ptr::null_mut();
                let mut nchildren: u32 = 0;
                let _ = xlib::XQueryTree(
                    self.display,
                    window,
                    &mut root,
                    &mut parent,
                    &mut children,
                    &mut nchildren,
                );
                if !children.is_null() {
                    // free children list
                    xlib::XFree(children as *mut _);
                }
                if parent == 0 || parent == root {
                    break;
                }
                window = parent;
            }
            window
        }
    }

    fn handle_button(&self, event: xlib::XEvent) {
        let event: XButtonPressedEvent = From::from(event);

        // get toplevel window if child was clicked
        let clicked_win = if event.subwindow != 0 {
            self.get_toplevel(event.subwindow)
        } else {
            event.subwindow
        };

        if clicked_win != 0 {
            unsafe {
                xlib::XSetInputFocus(
                    self.display,
                    clicked_win,
                    xlib::RevertToPointerRoot,
                    xlib::CurrentTime,
                );
                xlib::XRaiseWindow(self.display, clicked_win);
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
    }

    // TODO: maybe move it somewhere else
    // TODO: fix bar disappearing when window is dragged over it
    fn draw_bar(&self, content: Option<String>) {
        unsafe {
            let root = XDefaultRootWindow(self.display);

            let bar_str = match content {
                Some(text) => CString::new(text),
                None => CString::new(
                    self.bar_str
                        .replace("DESKTOP_HERE", &desktop_widget(self.current_desktop)),
                ),
            };

            let bar_str = match bar_str {
                Ok(stri) => stri,
                Err(e) => {
                    warn!(
                        "failed to create cstring for current desktop number: {}.",
                        e
                    );
                    return;
                }
            };

            xlib::XClearArea(self.display, root, 0, 0, self.screen_width as u32, 50, 0);
            xlib::XFillRectangle(
                self.display,
                root,
                self.background_gc,
                0,
                0,
                self.screen_width as u32,
                self.config.bar.height,
            );

            xlib::XDrawString(
                self.display,
                root,
                self.bar_gc,
                5,
                15,
                bar_str.as_ptr(),
                bar_str.to_bytes().len() as i32,
            );

            XFlush(self.display);
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

        let positions = self.tile(
            tileable.len(),
            self.screen_width as i32,
            self.screen_height as i32,
        );
        for (id, window) in tileable.iter().zip(positions) {
            self.resize_window(*id, window.w as u32, window.h as u32);
            self.move_window(*id, window.x, window.y);
        }
    }

    fn tile(&self, n: usize, screen_w: i32, screen_h: i32) -> Vec<WindowInfo> {
        let cols = (n as f32).sqrt().ceil() as i32;
        let rows = (n as i32 + cols - 1) / cols;
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
        if self.floating_windows.contains(&window) {
            return false;
        }

        unsafe {
            let window_type_c = CString::new("_NET_WM_WINDOW_TYPE").unwrap();
            let notif_c = CString::new("_NET_WM_WINDOW_TYPE_NOTIFICATION").unwrap();
            let dock_c = CString::new("_NET_WM_WINDOW_TYPE_DOCK").unwrap();
            let dialog_c = CString::new("_NET_WM_WINDOW_TYPE_DIALOG").unwrap();
            let splash_c = CString::new("_NET_WM_WINDOW_TYPE_SPLASH").unwrap();

            let window_type = xlib::XInternAtom(self.display, window_type_c.as_ptr(), 0);
            let notification_type = xlib::XInternAtom(self.display, notif_c.as_ptr(), 0);
            let dock_type = xlib::XInternAtom(self.display, dock_c.as_ptr(), 0);
            let dialog_type = xlib::XInternAtom(self.display, dialog_c.as_ptr(), 0);
            let splash_type = xlib::XInternAtom(self.display, splash_c.as_ptr(), 0);

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
                xlib::XFree(prop as *mut _);
                return wtype != notification_type
                    && wtype != dock_type
                    && wtype != dialog_type
                    && wtype != splash_type;
            }

            if !prop.is_null() {
                xlib::XFree(prop as *mut _);
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

    fn scale_down(&mut self, event: xlib::XButtonEvent) {
        if event.subwindow == 0 {
            return;
        }

        let win = self.get_toplevel(event.subwindow);

        self.floating_windows.insert(win);

        self.layout();

        unsafe {
            xlib::XSetInputFocus(
                self.display,
                win,
                xlib::RevertToPointerRoot,
                xlib::CurrentTime,
            );
            xlib::XRaiseWindow(self.display, win);
        }

        let attr = self.get_window_attributes(win);
        let new_width = (attr.width as u32 - self.config.general.scale_steps).max(100);
        let new_height = (attr.height as u32 - self.config.general.scale_steps).max(100);

        self.resize_window(win, new_width, new_height);
        self.move_window(
            win,
            attr.x + (attr.width - new_width as i32) / 2,
            attr.y + (attr.height - new_height as i32) / 2,
        );
    }

    fn scale_up(&mut self, event: xlib::XButtonEvent) {
        if event.subwindow == 0 {
            return;
        }

        let win = self.get_toplevel(event.subwindow);

        self.floating_windows.insert(win);

        self.layout();

        unsafe {
            xlib::XSetInputFocus(
                self.display,
                win,
                xlib::RevertToPointerRoot,
                xlib::CurrentTime,
            );
            xlib::XRaiseWindow(self.display, win);
        }

        let attr = self.get_window_attributes(win);
        // don't let the user make it too large
        let new_width = (attr.width as u32 + self.config.general.scale_steps)
            .min((self.screen_width as f32 * 1.5) as u32);
        let new_height = (attr.height as u32 + self.config.general.scale_steps)
            .min((self.screen_height as f32 * 1.5) as u32);

        self.resize_window(win, new_width, new_height);
        self.move_window(
            win,
            attr.x - ((new_width as i32 - attr.width) / 2),
            attr.y - ((new_height as i32 - attr.height) / 2),
        );
    }

    fn handle_drag_start(&mut self, event: xlib::XButtonEvent) {
        if event.subwindow == 0 {
            return;
        }

        // use toplevel window instead of child subwindow
        let win = self.get_toplevel(event.subwindow);

        self.floating_windows.insert(win);

        self.layout();

        let attr = self.get_window_attributes(win);

        self.drag_state = Some(DragState {
            window: win,
            start_win_x: attr.x,
            start_win_y: attr.y,
            start_mouse_x: event.x_root,
            start_mouse_y: event.y_root,
        });

        unsafe {
            // focus toplevel window
            xlib::XSetInputFocus(
                self.display,
                win,
                xlib::RevertToPointerRoot,
                xlib::CurrentTime,
            );
            xlib::XRaiseWindow(self.display, win);

            // grab pointer
            let root = XDefaultRootWindow(self.display);
            xlib::XGrabPointer(
                self.display,
                root,
                0, // owner_events false — handle events in WM
                (xlib::Button1MotionMask | xlib::ButtonReleaseMask) as u32,
                xlib::GrabModeAsync,
                xlib::GrabModeAsync,
                0,
                0,
                xlib::CurrentTime,
            );

            // replay pointer
            xlib::XAllowEvents(self.display, xlib::ReplayPointer, xlib::CurrentTime);
        }
    }

    fn handle_motion(&mut self, event: xlib::XMotionEvent) {
        if let Some(state) = self.drag_state {
            let delta_x = event.x_root - state.start_mouse_x;
            let delta_y = event.y_root - state.start_mouse_y;

            let new_x = state.start_win_x + delta_x;
            let new_y = state.start_win_y + delta_y;

            self.move_window(state.window, new_x, new_y);
        }
    }

    fn handle_release(&mut self, _event: xlib::XButtonEvent) {
        unsafe {
            xlib::XUngrabPointer(self.display, xlib::CurrentTime);
            // make sure pointer events are not blocked
            xlib::XAllowEvents(self.display, xlib::AsyncPointer, xlib::CurrentTime);
        }
        self.drag_state = None;
    }
}

// returns (background_gc, bar_gc)
fn create_bar_gc(
    display: *mut x11::xlib::_XDisplay,
    config: &Config,
) -> Option<(xlib::GC, xlib::GC)> {
    unsafe {
        let root = XDefaultRootWindow(display);
        let screen = XDefaultScreen(display);
        let mut bar_color: XColor = std::mem::zeroed();
        let mut background_color: XColor = std::mem::zeroed();

        let bar_hex_str = match CString::new(config.bar.text_color.clone()) {
            Ok(hex_str) => hex_str,
            Err(e) => {
                error!("failed to convert bar text color str to cstring: {}", e);
                return None;
            }
        };

        let background_hex_str = match CString::new(config.bar.background_color.clone()) {
            Ok(hex_str) => hex_str,
            Err(e) => {
                error!(
                    "failed to convert bar background color str to cstring: {}",
                    e
                );
                return None;
            }
        };

        if XParseColor(
            display,
            XDefaultColormap(display, screen),
            bar_hex_str.as_ptr(),
            &mut bar_color,
        ) != 1
        {
            error!("failed to parse bar text color");
        }

        if XParseColor(
            display,
            XDefaultColormap(display, screen),
            background_hex_str.as_ptr(),
            &mut background_color,
        ) != 1
        {
            error!("failed to parse bar background color");
        }

        XAllocColor(display, XDefaultColormap(display, screen), &mut bar_color);

        XAllocColor(
            display,
            XDefaultColormap(display, screen),
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
            display,
            root,
            GCForeground as u64,
            &bar_gcv as *const _ as *mut _,
        );

        let background_gc = xlib::XCreateGC(
            display,
            root,
            GCForeground as u64,
            &background_gcv as *const _ as *mut _,
        );

        Some((background_gc, bar_gc))
    }
}

fn get_screen_size(display: *mut xlib::Display) -> Result<(i16, i16), GridWMError> {
    unsafe {
        let mut num: i32 = 0;
        let screen_pointers = xinerama::XineramaQueryScreens(display, &mut num);
        let screens = slice::from_raw_parts(screen_pointers, num as usize).to_vec();
        let screen = screens.first();

        if let Some(screen) = screen {
            Ok((screen.width, screen.height))
        } else {
            Err(GridWMError::ScreenNotFound("0".to_string()))
        }
    }
}
