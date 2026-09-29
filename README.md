# vtt: Vertical Tabs Terminal

A lightweight, GPU-accelerated terminal with a vertical tab sidebar, inspired by Zen Browser's vertical tabs. It is meant for working with many terminals at once (AI agents, servers, builds) without losing track of them.

Written in Rust with [egui](https://github.com/emilk/egui) on [wgpu](https://wgpu.rs) (Vulkan / Metal / DX12). Terminal emulation comes from [alacritty_terminal](https://crates.io/crates/alacritty_terminal). No Electron and no webview.

## Features

- **Vertical tabs.** The sidebar can be collapsed to an icon strip.
- **Auto titles.** A tab shows the title the shell sets, or `<process> · <cwd>` (on Linux and macOS).
- **Activity indicators.** A dot appears when a background tab prints output, and a badge on a bell or when the process exits.
- **Multiple shells.** Shells are auto-discovered:
  - Linux/macOS: `$SHELL`, `/etc/shells` and `pwsh`
  - Windows: PowerShell 7, Windows PowerShell, cmd, Git Bash and WSL distros
  - You can also add your own profiles in the config.
- **Splits.** Drag a tab from the sidebar onto a pane:
  - Drop on an **edge** (left / right / top / bottom) to split that pane.
  - Drop in the **center** to swap the tab into that pane.
  - Each pane in a split has a header with `─` to **minimise** it (the pane goes back to being a standalone tab and the session stays alive) and `×` to **close** it.
  - Drag the dividers to resize panes.
- **Small footprint.** The app redraws only when something changes, so it uses about 0% CPU when idle.
- Runs on Linux (Wayland/X11), Windows (ConPTY) and macOS.

## Keybindings

On Linux and Windows the app shortcuts use **Alt**, the key in the same spot as Cmd on a Mac. Alt+B, Alt+D and Alt+F are left alone because shells use them to move and delete by word.

| Action | Linux / Windows | macOS |
|---|---|---|
| New tab | `Alt+T` (also `Ctrl+Shift+T`) | `⌘T` |
| Close tab | `Alt+W` (also `Ctrl+Shift+W`) | `⌘W` |
| Jump to tab 1–9 | `Alt+1` … `Alt+9` | `⌘1` … `⌘9` |
| Next / previous tab | `Ctrl+Tab` / `Ctrl+Shift+Tab`, `Ctrl+PageDown` / `Ctrl+PageUp` | same, plus `⌘⇧]` / `⌘⇧[` |
| Split right with new tab | `Alt+Shift+D` | `⌘D` |
| Split down with new tab | `Alt+Shift+E` | `⌘⇧D` |
| Toggle sidebar collapse | `Alt+Shift+B` | `⌘B` |
| Move focus between panes | `Alt+Arrow` (only while split) | `⌘⌥Arrow` |
| Font zoom in / out / reset | `Ctrl+=` / `Ctrl+-` / `Ctrl+0` | `⌘=` / `⌘-` / `⌘0` |
| Scroll back | `Shift+PageUp` / `Shift+PageDown` | same |
| Copy / paste | `Ctrl+Shift+C` / `Ctrl+Shift+V`, plus `Ctrl+V` | `⌘C` / `⌘V` |

`Ctrl+C` copies when text is selected; otherwise it sends an interrupt as usual.

Every shortcut can be changed in the `[keybindings]` section of the config (see below). Setting an action replaces all of its defaults. Use a list for several chords, and `[]` to unbind it. Modifiers are `ctrl`, `shift`, `alt` and `cmd` (macOS only). Keys are letters, digits, `f1`–`f24`, `tab`, `enter`, `space`, `pageup`, `pagedown`, `home`, `end`, `left`/`right`/`up`/`down`, `plus`, `minus`, `equals`, `[`, `]` and similar.

Actions: `new_tab`, `close_tab`, `split_right`, `split_down`, `toggle_sidebar`, `next_tab`, `prev_tab`, `goto_tab_1` … `goto_tab_9`, `focus_left`, `focus_right`, `focus_up`, `focus_down`, `zoom_in`, `zoom_out`, `zoom_reset`, `scroll_page_up`, `scroll_page_down`.

## Building

Requires a stable Rust toolchain.

```sh
cargo build --release
./target/release/vtt
```

On Linux, install the windowing/GPU development packages first (Debian/Ubuntu shown):

```sh
sudo apt install libxkbcommon-dev libwayland-dev libx11-dev libxcursor-dev \
  libxrandr-dev libxi-dev libgl1-mesa-dev libvulkan-dev libfontconfig1-dev
```

## Configuration

Config file location:

- Linux: `~/.config/vtt/config.toml`
- macOS: `~/Library/Application Support/vtt/config.toml`
- Windows: `%APPDATA%\vtt\config.toml`

Every key is optional. The example below shows the defaults. It is also available as [`config.example.toml`](config.example.toml).

```toml
# vtt configuration
# Linux:   ~/.config/vtt/config.toml
# macOS:   ~/Library/Application Support/vtt/config.toml
# Windows: %APPDATA%\vtt\config.toml
# Every key is optional; omitted keys use the defaults shown here.

# Lines of scrollback kept per tab.
scrollback = 10000

# Sidebar width in logical pixels when expanded.
sidebar_width = 240.0

# Start with the sidebar collapsed to icons.
sidebar_collapsed = false

# Profile used for new tabs (defaults to the first discovered shell).
# default_profile = "fish"

[font]
# Font family; defaults to the system monospace font.
# family = "JetBrains Mono"
size = 14.0

# Terminal palette (Catppuccin Mocha by default).
[colors]
foreground = "#cdd6f4"
background = "#1e1e2e"
cursor = "#f5e0dc"
selection = "#585b70"
# 16 ANSI colors: normal 0-7, then bright 8-15.
ansi = [
  "#45475a", "#f38ba8", "#a6e3a1", "#f9e2af", "#89b4fa", "#f5c2e7", "#94e2d5", "#bac2de",
  "#585b70", "#f38ba8", "#a6e3a1", "#f9e2af", "#89b4fa", "#f5c2e7", "#94e2d5", "#a6adc8",
]

# Shortcut overrides. Setting an action replaces all of its defaults.
# Use a list for several chords, or [] to unbind an action.
[keybindings]
# new_tab = "ctrl+t"                       # if you don't use Ctrl+T in your shell or fzf
# next_tab = ["ctrl+tab", "alt+j"]
# prev_tab = ["ctrl+shift+tab", "alt+k"]
# close_tab = []                           # unbind

# Extra profiles, added after the auto-discovered shells.
# A profile with the same name as a discovered one replaces it.
[[profiles]]
name = "htop"
command = "htop"
args = []
# cwd = "/home/me/projects"
icon = "H"
color = "#a6e3a1"
[profiles.env]
# FOO = "bar"
```

## License

MIT
