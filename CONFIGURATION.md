# GridWM Configuration

GridWM uses a TOML configuration file located at `~/.config/gridwm/gridwm.toml`.

## Sections

### `[start]`
Executed programs on WM startup.

- **`exec`** (array of strings): List of commands to run when GridWM starts.
  ```toml
  [start]
  exec = ["picom", "kitty"]
  ```

### `[keyboard]`
Keyboard settings.

- **`layout`** (string): Keyboard layout (e.g., `"us"`, `"de"`, `"fr"`). Empty string means no layout change.
  ```toml
  [keyboard]
  layout = "us"
  ```

### `[mouse]`
Mouse acceleration settings.

- **`use_acceleration`** (boolean): Enable mouse acceleration.
- **`use_acceleration_threshold`** (boolean): Whether to use the value for acceleration threshold.
- **`acceleration_value`** (number): Multiplier for mouse acceleration (e.g., `2.0` means 2x speed).
- **`acceleration_threshold`** (integer): Pixel threshold before acceleration kicks in.
  ```toml
  [mouse]
  use_acceleration = false
  use_acceleration_threshold = false
  acceleration_value = 2.0
  acceleration_threshold = 5
  ```

### `[desktop]`
Desktop appearance settings.

- **`color`** (string): Background color in hex format (e.g., `"#464646"`).
  ```toml
  [desktop]
  color = "#464646"
  ```

### `[bar]`
Status bar appearance settings.

- **`text_color`** (string): Text color in hex format (e.g., `"#ffffff"`).
- **`background_color`** (string): Bar background color in hex format (e.g., `"#272727"`).
- **`height`** (integer): Height of the status bar in pixels (default: `20`).
- **`enable`** (boolean): Enable or disable the status bar.
- **`update`** (number): Update interval for widgets on the bar.
- **`widgets`** (array of strings): Which widgets to enable. (desktop, time, cpu, mem, battery)
  ```toml
  [bar]
  text_color = "#ffffff"
  background_color = "#272727"
  height = 20
  enable = true
  update = 2.0
  widgets = ["desktop", "time", "cpu", "mem", "battery"]
  ```

### `[keybinds]` (gridwm keybinds)
Keybinds for window manager actions.

- **Format**: Array of arrays with two strings: `[["KEY_COMBINATION", "ACTION"], ...]`
- **Supported modifiers**: `CTRL`, `SHIFT`, `ALT`, `SUPER` (or `WIN`, `MOD4`)
- **Supported actions**: `close`, `desktop_right`, `desktop_left`
  ```toml
  [keybinds]
  gridwm = [
    ["SUPER+Q", "close"],
    ["SUPER+Right", "desktop_right"],
    ["SUPER+Left", "desktop_left"]
  ]
  ```

### `[keybinds]` (exec keybinds)
Keybinds for executing custom commands.

- **Format**: Array of arrays with two strings: `[["KEY_COMBINATION", "COMMAND"], ...]`
  ```toml
  [keybinds.]
  exec = [
    ["SUPER+Return", "alacritty"],
    ["SUPER+D", "dmenu_run"]
  ]
  ```

## Example Configuration

```toml
[start]
exec = ["firefox"]

[keyboard]
layout = "us"

[mouse]
use_acceleration = false
use_acceleration_threshold = false
acceleration_value = 2.0
acceleration_threshold = 5

[desktop]
color = "#464646"

[bar]
text_color = "#ffffff"
background_color = "#272727"
height = 20
update = 2.0
enable = true
widgets = ["desktop", "time", "cpu", "mem"]

[[keybinds.gridwm]]
gridwm = [
  ["SUPER+Q", "close"],
  ["SUPER+Right", "desktop_right"],
  ["SUPER+Left", "desktop_left"]
]

[[keybinds.exec]]
exec = [
  ["SUPER+Return", "alacritty"]
]
```

> Empty or missing configuration options will default to the built-in settings.