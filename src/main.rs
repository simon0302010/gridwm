mod gridwm;

use std::error::Error;

use gridwm::GridWM;

fn main() -> Result<(), Box<dyn Error>> {
    let display_name = std::env::var("DISPLAY")?;

    let mut wm = GridWM::new(&display_name)?;

    wm.init()?;
    wm.run();

    Ok(())
}
