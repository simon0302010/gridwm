mod gridwm;

use std::error::Error;

use gridwm::GridWM;

fn main() -> Result<(), Box<dyn Error>> {
    const SUPPORTED_OSES: &[&str] = &[
        "linux",
        "freebsd",
        "openbsd",
        "netbsd",
        "dragonfly",
        "solaris",
    ];

    if !SUPPORTED_OSES.contains(&std::env::consts::OS) {
        eprintln!("GridWM does not support {}", std::env::consts::OS);
        return Ok(());
    }

    let display_name = std::env::var("DISPLAY")?;

    let mut wm = GridWM::new(&display_name)?;

    wm.init()?;
    wm.run();

    Ok(())
}
