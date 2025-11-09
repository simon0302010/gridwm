# GridWM

GridWM is a tiling X11 based window manager for Linux. This project is still in its early stages of development and lacks many features.
> ⚠️ This project does not support Windows or macOS.

## Features

- Basic tiling window management
- Customizable keyboard shortcuts for managing windows and workspaces
- Customizable keyboard shortcuts for running custom commands
- Easy to configure using a config file
- Multi-desktop support
- Lightweight
- Status bar that shows current desktop and time
- Written in Rust

## Todo

- Allow user to move and resize windows
- Add support for notifications
- Add support for multi-monitor setups
- Improve status bar with more information

## Installation

1. Install Dependencies

   Debian based systems:

   ```bash
   sudo apt install xserver-xorg xinit x11-xserver-utils xorg-setxkbmap xorg
   ```

   Arch based systems:

   ```bash
   sudo pacman -S xorg-server xorg-xinit xorg-setxkbmap xorg-xsetroot
   ```

2. Install Binary

   You can download the latest release binary from the [releases page](https://github.com/simon0302010/gridwm/releases) and place it in a directory included in your system's PATH, such as `/usr/local/bin`.

    Alternatively, you can build from source:
    
    ```bash
    cargo install --git https://github.com/simon0302010/gridwm.git
    ```
    > ⚠️ Make sure you have Rust and Cargo installed on your system and added `~/.cargo/bin` added to your PATH before running the above command.
