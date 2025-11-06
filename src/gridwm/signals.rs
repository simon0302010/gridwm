use std::ffi::CString;

use x11::xlib::{ClientMessage, CurrentTime, Display, NoEventMask, XEvent, XInternAtom, XSendEvent};

use crate::gridwm::Window;

pub fn send_wm_delete_window(display: *mut Display, window: Window) {
    unsafe {
        let mut event: XEvent = std::mem::zeroed();
        let xclient = &mut event.client_message;
        xclient.type_ = ClientMessage;
        xclient.window = window;
        let wm_protocols = CString::new("WM_PROTOCOLS").unwrap();
        xclient.message_type = XInternAtom(display, wm_protocols.as_ptr(), 1);
        xclient.format = 32;
        let wm_delete_window = CString::new("WM_DELETE_WINDOW").unwrap();
        let wm_delete_atom = XInternAtom(display, wm_delete_window.as_ptr(), 0);
        xclient.data.set_long(0, wm_delete_atom as i64);
        xclient.data.set_long(1, CurrentTime as i64);

        XSendEvent(display, window, 0, NoEventMask, &mut event);
    }
}