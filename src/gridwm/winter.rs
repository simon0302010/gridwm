use std::ffi::CString;

use rand::Rng;
use x11::xlib::{Display, GC, XClearArea, XDefaultRootWindow, XDrawString};

#[derive(Debug)]
pub struct Snowflake {
    pub x: f32,
    pub y: f32,
    pub speed: f32,
    pub drift: f32,
}

pub fn draw_snowflakes(
    display: *mut Display,
    snowflakes: &mut Vec<Snowflake>,
    screen_width: i16,
    screen_height: i16,
    flake_gc: GC,
) {
    unsafe {
        let root = XDefaultRootWindow(display);

        for flake in snowflakes {
            XClearArea(
                display,
                root,
                flake.x as i32 - 2,
                flake.y as i32 - 14,
                18,
                20,
                0,
            );

            flake.y += flake.speed;
            flake.x += flake.drift;

            if flake.y > screen_height as f32 {
                flake.y = 0.0;
            }
            if flake.x < 0.0 {
                flake.x = screen_width as f32
            } else if flake.x > screen_width as f32 {
                flake.x = 0.0
            }

            let flake_str = CString::new("*").unwrap_or_default();
            XDrawString(
                display,
                root,
                flake_gc,
                flake.x as i32,
                flake.y as i32,
                flake_str.as_ptr(),
                flake_str.to_bytes().len() as i32,
            );
        }
    }
}

// snowflake generation
pub fn generate_snowflakes(screen_width: i16, screen_height: i16) -> Vec<Snowflake> {
    let mut rng = rand::rng();

    (0..40)
        .map(|_| Snowflake {
            x: rng.random_range(0.0..=screen_width as f32),
            y: rng.random_range(0.0..=screen_height as f32),
            speed: rng.random_range(0.5..=1.8),
            drift: rng.random_range(-0.3..=0.3),
        })
        .collect()
}
