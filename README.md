# vtt: Vertical Tabs Terminal

A lightweight, GPU-accelerated terminal with a vertical tab sidebar, inspired by Zen Browser's vertical tabs. It is meant for working with many terminals at once (AI agents, servers, builds) without losing track of them.

Written in Rust with [egui](https://github.com/emilk/egui) on [wgpu](https://wgpu.rs) (Vulkan / Metal / DX12). Terminal emulation comes from [alacritty_terminal](https://crates.io/crates/alacritty_terminal). No Electron and no webview.

## Features

- **Vertical tabs.** The sidebar can be collapsed to an icon strip.
- **Folders.** Group tabs into named, colored folders that collapse, Zen-style (see [Folders](#folders)).
- **Files panel.** A file tree of the focused shell's directory sits between the tabs and the terminals, and follows you as you `cd` (see [Files and previews](#files-and-previews)).
- **File previews.** Click a file to open it in a pane next to the terminal: rendered Markdown, syntax-highlighted code and JSON, images.
- **Hover a tab** for a moment to see its full working directory and the program running in it.
- **Auto titles.** A tab shows the title the shell sets, or `<process> · <cwd>` (on Linux and macOS).
- **Activity indicators.** A dot appears when a background tab prints output, and a badge on a bell or when the process exits.
- **Multiple shells.** Shells are auto-discovered:
  - Linux/macOS: `$SHELL`, `/etc/shells` and `pwsh`
  - Windows: PowerShell 7, Windows PowerShell, cmd, Git Bash and WSL distros
  - You can also add your own profiles in the config.
- **Splits.** Drag a tab from the sidebar onto a pane:
  - Drop on an **edge** (left / right / top / bottom) to split that pane.
  - Drop in the **center** to swap the tab into that pane.
  - Each pane in a split has a header with `—` to **minimise** it (the pane goes back to being a standalone tab and the session stays alive) and `×` to **close** it.
  - Drag the dividers to resize panes.
- **Small footprint.** The app redraws only when something changes, so it uses about 0% CPU when idle.
- Runs on Linux (Wayland/X11), Windows (ConPTY) and macOS.

## Keybindings

On Linux and Windows, vtt only binds the shortcuts terminals conventionally use (`Ctrl+Shift+T`, `Ctrl+Tab`, …). Alt chords are left to your shell. Every other action is in the command palette, and you can give it a key in the config (see below). On macOS, the usual ⌘ shortcuts are bound. In the tables, "—" means no default.

**Moving around**

| Action | Linux / Windows | macOS |
|---|---|---|
| Tab switcher / command palette | `Ctrl+Shift+P` | `⌘P` / `⌘⇧P` |
| Next / previous tab (in sidebar order, including split panes) | `Ctrl+Tab` / `Ctrl+Shift+Tab`, `Ctrl+PageDown` / `Ctrl+PageUp` | same, plus `⌘↓` / `⌘↑` and `⌘⇧]` / `⌘⇧[` |
| Jump to tab 1–8 / last tab | — | `⌘1` … `⌘8` / `⌘9` |
| Go to next tab with new output or a bell | — | `⌘⇧A` |
| Focus pane left / right / up / down | — | `⌘⌥←` / `⌘⌥→` / `⌘⌥↑` / `⌘⌥↓` |

**Managing tabs**

| Action | Linux / Windows | macOS |
|---|---|---|
| New tab | `Ctrl+Shift+T` | `⌘T` |
| Close tab | `Ctrl+Shift+W` | `⌘W` |
| Reopen closed tab (same shell, folder, name and position) | — | `⌘⇧T` |
| Rename tab | — | `⌘R` |
| Move tab (or its split group) up / down | — | `⌘⇧↑` / `⌘⇧↓` |
| Split right / down with a new tab | — | `⌘D` / `⌘⇧D` |
| Minimise pane back to its own tab | — | `⌘⇧M` |
| Toggle sidebar collapse | — | `⌘B` |
| Toggle files panel | `Ctrl+Shift+E` | `⌘⇧E` |
| New folder with the focused tab | — | — |
| Duplicate tab | — | — |

**Terminal**

| Action | Linux / Windows | macOS |
|---|---|---|
| Font zoom in / out / reset | `Ctrl+=` / `Ctrl+-` / `Ctrl+0` | `⌘=` / `⌘-` / `⌘0` |
| Scroll back | `Shift+PageUp` / `Shift+PageDown` | same |
| Open settings file | `Ctrl+,` | `⌘,` |
| Copy / paste | `Ctrl+Shift+C` / `Ctrl+Shift+V`, plus `Ctrl+V` | `⌘C` / `⌘V` |

`Ctrl+C` copies when text is selected; otherwise it sends an interrupt as usual. Pane-focus and next/previous-tab keys pass through to the shell when there is nothing to move to (a single tab, or no pane in that direction).

**The palette** (`Ctrl+Shift+P`) fuzzy-searches your tabs, every command (each shown with its shortcut, so it doubles as a cheat sheet) and "new tab with profile X".
- It opens with your previously focused tab selected, so `Ctrl+Shift+P`, `Enter` flips between two tabs.
- Move with `Up`/`Down`, `Tab`/`Shift+Tab` or `Ctrl+N`/`Ctrl+P`; `Enter` picks and `Esc` closes.

Every shortcut can be changed in the `[keybindings]` section of the config (see below). Setting an action replaces all of its defaults. Use a list for several chords, and `[]` to unbind it. Modifiers are `ctrl`, `shift`, `alt` and `cmd` (macOS only). Keys are letters, digits, `f1`–`f24`, `tab`, `enter`, `space`, `pageup`, `pagedown`, `home`, `end`, `left`/`right`/`up`/`down`, `plus`, `minus`, `equals`, `[`, `]` and similar.

Actions: `new_tab`, `close_tab`, `reopen_closed_tab`, `duplicate_tab`, `rename_tab`, `split_right`, `split_down`, `minimize_pane`, `toggle_sidebar`, `next_tab`, `prev_tab`, `move_tab_up`, `move_tab_down`, `goto_tab_1` … `goto_tab_9`, `last_tab`, `next_activity`, `command_palette`, `toggle_files`, `new_group`, `open_settings`, `focus_left`, `focus_right`, `focus_up`, `focus_down`, `zoom_in`, `zoom_out`, `zoom_reset`, `scroll_page_up`, `scroll_page_down`.

## Files and previews

The files panel (`Ctrl+Shift+E`, `⌘⇧E` on macOS, or 🗀 in the sidebar header) shows the directory of the focused shell. When you `cd`, it follows. Switching tabs shows that tab's directory.

- **Click a folder** to expand it; **double-click** it to `cd` the shell there. If a program is running in the shell, a new tab opens in that folder instead of typing into the program.
- **Click a file** to preview it in a pane beside the terminal. Clicking other files reuses that pane, so previews don't pile up. Keyboard focus stays in the terminal.
- **Drag a file** onto a terminal to type its (quoted) path.
- **Right-click** for: cd into a folder, new tab there, browse there, insert the path, copy the path, open with the default app.
- The header has ⬆ (browse the parent until the shell changes directory), `.*` (show hidden files) and ⊟ (collapse all).

Previews are tabs like any other: drag them, split them, close them, reopen them with "Reopen closed tab" (`⌘⇧T` on macOS). They reload when the file changes on disk.

- **Markdown** renders with headings, lists, tables, links, local images, and highlighted code blocks. **Source** in the preview toolbar shows the raw text.
- **Code and text** are syntax-highlighted in your theme's colors, with line numbers. Long lines wrap at word boundaries so narrow panes stay readable; **Wrap** in the toolbar switches that off (and `wrap = false` under `[files]` makes it the default). That covers Rust, Python, JS/TS, Go, C/C++, shell, TOML, YAML, JSON and many more. Minified JSON is reformatted for reading.
- **Images** (PNG, JPEG, GIF, WebP) are shown fitted to the pane.
- Binary and very large files show their size and a button to open them with the default app.

## Folders

Folders group tabs in the sidebar, like folders in Zen Browser.

- Right-click a tab and pick **New folder with tab** (or run it from the palette), then type a name.
- Click a folder's header to collapse or expand it. A collapsed folder still shows the tab you're on, plus how many tabs it holds and a dot if one of them has new output.
- To add a tab to a folder, drag it onto the folder's header or between its tabs, or use **Move to folder**. Drag a tab out (or use **Remove from folder**) to take it out.
- New tabs opened from a tab in a folder join that folder. A split view always moves as a unit.
- Right-click a folder's header to rename it, change its color, open a new tab in it, ungroup it, or close all its tabs.

## Installing

**Arch / CachyOS / Manjaro:** build and install a package from this checkout:

```sh
cd packaging/arch
makepkg -si
```

It packages the latest *committed* state and installs `vtt` to `/usr/bin` with a desktop entry, so it shows up in your app launcher. If your Rust toolchain came from rustup's install script rather than pacman, use `makepkg -sid` so makepkg doesn't pull in the `rust` package as a build dependency. To update, pull (or commit) and run `makepkg -si` again. Remove it with `sudo pacman -R vtt-git`.

**Anywhere with Rust:**

```sh
cargo install --path .
```

This installs `vtt` to `~/.cargo/bin`. For your app launcher, copy the desktop entry too: `cp packaging/vtt.desktop ~/.local/share/applications/` (it runs `vtt`, so `~/.cargo/bin` must be on the launcher's `PATH`).

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

The easiest way in is **Open settings**: press `Ctrl+,` (`⌘,` on macOS), click ⚙ in the sidebar header, or pick it from the palette.
- If the file doesn't exist yet, it's created from the commented example below.
- It opens in a new tab running your editor (the `editor` setting, `$VISUAL` or `$EDITOR`), and the tab closes when you quit the editor.
- Without an editor configured, it opens in your system's default text editor.

Changes apply as soon as you save; there's no need to restart. If something in the file is wrong (a syntax error, an unknown theme, a bad shortcut), a banner in the bottom-right corner says what and where. It disappears once the file is fixed. While the config has a syntax error, vtt keeps your previous settings.

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

# Color theme: a built-in (default, catppuccin-latte, gruvbox-dark, tokyo-night) or the name of
# a file in the themes folder next to this config (<name>.toml, or <name>.conf in kitty format).
# theme = "tokyo-night"

# Or point at any theme file directly (vtt .toml, or kitty format for anything else).
# It is watched: when the file changes, vtt recolors live.
# theme_file = "~/.config/vtt/themes/mine.toml"

# When a shell sets colors via escape sequences (pywal, wallust and many wallpaper scripts do
# this for every open terminal), apply them to the whole app instead of just that tab.
adopt_shell_palette = false

# Editor for "Open settings" (Ctrl+,). Defaults to $VISUAL / $EDITOR, then the system's default app.
# Terminal editors open in a new vtt tab.
# editor = "nvim"

[font]
# Font family; defaults to the system monospace font.
# family = "JetBrains Mono"
size = 14.0

[files]
# Show the files panel (between the tabs and the terminals) at startup. Toggle: Ctrl+Shift+E.
open = false
# Initial width in logical pixels; drag the panel's edge to resize.
width = 260.0
# List dotfiles (and hidden files on Windows).
show_hidden = false
# Wrap long lines in text and code previews (toggle with "Wrap" in the preview's toolbar).
wrap = true

# Inline color overrides, applied on top of the theme. Same keys as a theme file:
# foreground, background, cursor, selection, ansi = [16 colors], colorN (any index 0-255),
# and [colors.ui] accent / sidebar.
[colors]
# background = "#101010"
# color4 = "#7aa2f7"

# Shortcut overrides. Setting an action replaces all of its defaults.
# Use a list for several chords, or [] to unbind an action.
[keybindings]
# new_tab = "ctrl+t"                       # if you don't use Ctrl+T in your shell or fzf
# next_tab = ["ctrl+tab", "alt+j"]
# prev_tab = ["ctrl+shift+tab", "alt+k"]
# close_tab = []                           # unbind
# duplicate_tab = "alt+shift+n"            # unbound by default
# new_group = "alt+g"                      # unbound by default
# goto_tab_1 = "alt+1"                     # Linux/Windows: most actions are unbound

# Extra profiles, added after the auto-discovered shells.
# A profile with the same name as a discovered one replaces it.
# [[profiles]]
# name = "htop"
# command = "htop"
# args = []
# cwd = "/home/me/projects"
# icon = "H"
# color = "#a6e3a1"
# [profiles.env]
# FOO = "bar"
```

## Themes

Colors come from layers, lowest to highest:

1. The built-in default (Catppuccin Mocha).
2. `theme = "<name>"` or `theme_file = "<path>"`.
   - Built-in themes: `default`, `catppuccin-latte`, `gruvbox-dark`, `tokyo-night`.
   - Any other name loads `<name>.toml` or `<name>.conf` from the `themes/` folder next to your config.
3. `[colors]` in the config, for one-off overrides.
4. With `adopt_shell_palette = true`, colors a shell sets at runtime (see below).

Theme files come in two formats:

- **vtt TOML** (`.toml`) uses the same keys as `[colors]`:
  ```toml
  foreground = "#c0caf5"
  background = "#1a1b26"
  cursor = "#c0caf5"
  selection = "#33467c"
  ansi = ["#15161e", "#f7768e", "#9ece6a", "#e0af68", "#7aa2f7", "#bb9af7", "#7dcfff", "#a9b1d6",
          "#414868", "#f7768e", "#9ece6a", "#e0af68", "#7aa2f7", "#bb9af7", "#7dcfff", "#c0caf5"]
  color232 = "#101010"   # any palette index
  [ui]
  accent = "#7aa2f7"     # optional; defaults to color4
  sidebar = "#16161e"    # optional; defaults to a shade of the background
  ```
- **kitty format** (any other extension). `background`, `foreground`, `cursor`, `selection_background`, `colorN` and `active_border_color` (used as the accent) are read; everything else is ignored. Existing kitty themes and generated theme files therefore work unchanged.

The sidebar, pane headers and palette derive their colors from the theme, so light themes work too.

**Live reload.** The config file and the active theme file are checked about once a second. Edits apply immediately to every tab and the UI. A config with a syntax error is reported on stderr and ignored until it's fixed.

### Wallpaper-generated themes (pywal, wallust, matugen, …)

vtt doesn't know about any of these tools; it only reads files and escape sequences, so any of them can drive it:

- **Via a file (recommended).** Point `theme_file` at a kitty-format file your tool already writes, or add a template to your tool that writes a vtt `.toml` file. vtt reloads whenever the file changes, and the colors are correct from startup.
  ```toml
  # e.g. end-4 dots-hyprland / illogical-impulse (matugen):
  theme_file = "~/.local/state/quickshell/user/generated/terminal/kitty-theme.conf"
  # e.g. pywal:
  # theme_file = "~/.cache/wal/colors-kitty.conf"
  ```
- **Via escape sequences.** Many of these scripts recolor terminals by writing OSC 4/10/11 sequences to every open terminal. With `adopt_shell_palette = true`, vtt applies such colors to the whole app instead of only the tab that received them. Trade-offs:
  - Any program that sets colors this way (some vim colorschemes, remote sessions) will also recolor the app.
  - Colors adopted this way aren't saved, so after a restart you'll see the theme from your config until the next update.

A matugen template for a vtt TOML theme (add it to `~/.config/matugen/config.toml` with an `output_path` in `~/.config/vtt/themes/`):

```toml
foreground = "{{colors.on_surface.default.hex}}"
background = "{{colors.surface.default.hex}}"
cursor = "{{colors.on_surface.default.hex}}"
selection = "{{colors.secondary_container.default.hex}}"
[ui]
accent = "{{colors.primary.default.hex}}"
sidebar = "{{colors.surface_container_low.default.hex}}"
```

This only sets the base and UI colors; the 16 ANSI colors stay from the default theme unless you add `colorN` lines too. Material You doesn't define ANSI colors, which is why wallpaper setups usually generate a separate terminal palette.

## License

MIT
