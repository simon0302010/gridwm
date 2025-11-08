use x11::xlib;

pub fn parse_keybind(display: *mut xlib::Display, keys: String) -> Option<(u32, i32)> {
    let keys = keys.split("+").map(|s| s.trim()).collect::<Vec<&str>>();

    let mut mask = 0;
    let mut key: Option<String> = None;

    for k in keys {
        let k_upper = k.to_uppercase();
        match k_upper.as_str() {
            "CTRL" | "CONTROL" => mask |= xlib::ControlMask,
            "SHIFT" => mask |= xlib::ShiftMask,
            "ALT" => mask |= xlib::Mod1Mask,
            "SUPER" | "WIN" | "WINDOWS" | "MOD4" => mask |= xlib::Mod4Mask,
            _ => key = Some(k_upper.clone()),
        }
    }

    if let Some(k) = key {
        let keysym = if k.len() == 1 {
            k.chars().next().unwrap() as u32
        } else {
            match k.as_str() {
                "SPACE" => x11::keysym::XK_space,
                "TAB" => x11::keysym::XK_Tab,
                "ENTER" => x11::keysym::XK_Return,
                "RIGHT" => x11::keysym::XK_Right,
                "LEFT" => x11::keysym::XK_Left,
                "UP" => x11::keysym::XK_Up,
                "DOWN" => x11::keysym::XK_Down,
                _ => return None,
            }
        };
        let keycode = unsafe { xlib::XKeysymToKeycode(display, keysym as u64) as i32 };
        Some((mask, keycode))
    } else {
        None
    }
}
