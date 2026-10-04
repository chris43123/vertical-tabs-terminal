# vtt: Vertical Tabs Terminal

A lightweight, GPU-accelerated terminal with a vertical tab sidebar, inspired by Zen Browser's vertical tabs. It is meant for working with many terminals at once (AI agents, servers, builds) without losing track of them.

Written in Rust with [egui](https://github.com/emilk/egui) on [wgpu](https://wgpu.rs) (Vulkan / Metal / DX12). Terminal emulation comes from [alacritty_terminal](https://crates.io/crates/alacritty_terminal). No Electron and no webview.

## Why vtt

vtt is a terminal first, built for a CLI-agent-first workflow. It isn't an editor.

- **Many projects at once.** In DevOps work you often have several projects open at the same time, and they are often related: infrastructure, services, pipelines, their configs. Each lives in its own terminal tab or folder of tabs, and vtt is built to keep track of many of them.
- **Agents do the editing.** Files are rarely edited by hand any more; CLI agents running in terminal tabs do it. What you need is to watch them work: what's running where, which files changed, what's on which branch.
- **Glance at files without an IDE.** Opening VS Code just to look at a few files, and to get a decent terminal, costs a lot of resources. vtt gives you a file tree, previews, git status and file search next to your terminals, and stops there.
- **Slim, on purpose.** It's a lightweight native app that sits at about 0% CPU when idle. A feature has to help you run and watch work in terminals. Editing belongs to your agents, or to your own editor running in a pane, never to vtt itself. Where a good command-line tool already exists (like `git`), vtt runs it instead of reimplementing it.
- **Standard keys.** Shortcuts are the conventional ones, and everything else is in the palette, so vtt doesn't take keys from your shell or tools.

Inspired by Warp, minus the weight: Warp's good ideas (a file tree that follows your shell, git at a glance, previews), with vertical tabs from Zen Browser, kept small.

## Features

- **Vertical tabs.** The sidebar can be collapsed to an icon strip, or hidden entirely (zen mode, `Ctrl+B`) and peeked by hovering the left window edge (`Ctrl+\` in zen mode switches the peek between tabs and files).
- **Folders.** Group tabs into named, colored folders that collapse, Zen-style (see [Folders](#folders)).
- **Files panel.** A file tree of the focused shell's directory sits between the tabs and the terminals, and follows you as you `cd` (see [Files and previews](#files-and-previews)).
- **File previews.** Click a file to open it in a pane next to the terminal: rendered Markdown, syntax-highlighted code and JSON, images.
- **Git at a glance.** The files panel shows the branch, commits ahead/behind, and changed files highlighted in green.
- **File search.** Fuzzy-find any file in the project from the files panel.
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
- **Accessibility.** Zoom the whole UI or a single pane, minimum text contrast, line height and letter spacing, cursor shape/blink/thickness, reduced motion, a focus border for splits and an optional visual bell.
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
| Toggle sidebar collapse | — | — |
| Zen mode: hide sidebar and files panel (hover the left edge to peek; `Ctrl+\` while in zen switches the peek between tabs and files) | `Ctrl+B` | `⌘B` |
| Toggle files panel | `Ctrl+\` or `Ctrl+Shift+E` | `⌘\` or `⌘⇧E` |
| New folder with the focused tab | — | — |
| Search files (in the files panel) | — | — |
| Duplicate tab | — | — |

**Terminal**

| Action | Linux / Windows | macOS |
|---|---|---|
| Zoom focused pane in / out / reset | `Ctrl+=` / `Ctrl+-` / `Ctrl+0` | `⌘=` / `⌘-` / `⌘0` |
| Zoom whole UI in / out / reset | `Ctrl+Shift+=` / `Ctrl+Shift+-` / `Ctrl+Shift+0` | `⌘⇧=` / `⌘⇧-` / `⌘⇧0` |
| Scroll back | `Shift+PageUp` / `Shift+PageDown` | same |
| Open settings file | `Ctrl+,` | `⌘,` |
| Keyboard shortcuts: every shortcut and mouse gesture, click one to rebind it | `Ctrl+Shift+/` (`Ctrl+?`) | `⌘⇧/` (`⌘?`) |
| Copy / paste | `Ctrl+Shift+C` / `Ctrl+Shift+V`, plus `Ctrl+V` | `⌘C` / `⌘V` |

`Ctrl+C` copies when text is selected; otherwise it sends an interrupt as usual. Pane-focus and next/previous-tab keys pass through to the shell when there is nothing to move to (a single tab, or no pane in that direction).

**The palette** (`Ctrl+Shift+P`) fuzzy-searches your tabs, every command (each shown with its shortcut, so it doubles as a cheat sheet) and "new tab with profile X".
- It opens with your previously focused tab selected, so `Ctrl+Shift+P`, `Enter` flips between two tabs.
- Move with `Up`/`Down`, `Tab`/`Shift+Tab` or `Ctrl+N`/`Ctrl+P`; `Enter` picks and `Esc` closes.

Every shortcut can be changed in the `[keybindings]` section of the config (see below). Setting an action replaces all of its defaults. Use a list for several chords, and `[]` to unbind it. Modifiers are `ctrl`, `shift`, `alt` and `cmd` (macOS only). Keys are letters, digits, `f1`–`f24`, `tab`, `enter`, `space`, `pageup`, `pagedown`, `home`, `end`, `left`/`right`/`up`/`down`, `plus`, `minus`, `equals`, `[`, `]` and similar.

Actions: `new_tab`, `close_tab`, `reopen_closed_tab`, `duplicate_tab`, `rename_tab`, `split_right`, `split_down`, `minimize_pane`, `toggle_sidebar`, `toggle_side_area`, `next_tab`, `prev_tab`, `move_tab_up`, `move_tab_down`, `goto_tab_1` … `goto_tab_9`, `last_tab`, `next_activity`, `command_palette`, `toggle_files`, `search_files`, `new_group`, `open_settings`, `focus_left`, `focus_right`, `focus_up`, `focus_down`, `zoom_in`, `zoom_out`, `zoom_reset`, `ui_zoom_in`, `ui_zoom_out`, `ui_zoom_reset`, `scroll_page_up`, `scroll_page_down`, `show_help`.

The easiest way to rebind is the **keyboard shortcuts** window (`Ctrl+Shift+/`). Click a shortcut and press the new one (`Backspace` removes it, `Esc` cancels), or `+` to add another. It writes the `[keybindings]` line for you and keeps the rest of the file as it is. Shortcuts bound to more than one action are shown in red.

## Files and previews

The files panel (`Ctrl+\` or `Ctrl+Shift+E`, `⌘\` or `⌘⇧E` on macOS, or 🗀 in the sidebar header) shows the directory of the focused shell. When you `cd`, it follows. Switching tabs shows that tab's directory.

- **Click a folder** to expand it; **double-click** it to `cd` the shell there. If a program is running in the shell, a new tab opens in that folder instead of typing into the program.
- **Click a file** to preview it in a pane beside the terminal. Clicking other files reuses that pane, so previews don't pile up. Keyboard focus stays in the terminal.
- **Drag a file** onto a pane, like dragging a tab:
  - Drop it on an **edge** to open it in a new split there, so two files (or a file and a terminal) sit side by side. A folder dropped on an edge opens a terminal in that folder.
  - Drop it in the **center** of a terminal to type its (quoted) path, or of a preview to show it there instead.
  - Drop it on the **tab list** to open it as a tab of its own.
- **Right-click** for: cd into a folder, new tab there, browse there, insert the path, copy the path, open with the default app.
- The header has ⬆ (browse the parent until the shell changes directory), `.*` (show hidden files) and ⊟ (collapse all).
- **Git:** inside a repository, the header shows the branch, ⏶/⏷ for commits ahead of/behind its upstream, and how many files changed. Changed and new files, and the folders holding them, are green in the tree (conflicts are red). It runs your own `git` in the background, at most every few seconds and right after a command finishes in the focused shell, and never takes git's index lock, so it won't get in the way of git commands you or an agent run.
- **Search:** type in the box above the tree (or run "Search files" from the palette) to fuzzy-find any file below the folder. In a repository it searches what git tracks plus new files, skipping ignored ones; elsewhere it skips hidden folders and `node_modules`, `target` and similar. `Up`/`Down` pick a result, `Enter` previews it, `Esc` clears the search. Results can be dragged like files in the tree.

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

Prebuilt packages are attached to each [release](https://github.com/chris43123/vertical-tabs-terminal/releases):

| Platform | Package | |
|---|---|---|
| macOS (Apple Silicon + Intel) | `vtt-<version>-macos-universal.dmg` | Open it and drag vtt to Applications. |
| | `vtt-<version>-macos-universal.zip` | The same app, zipped. |
| Windows (x64) | `vtt-<version>-windows-x86_64-setup.exe` | Installer: Start menu entry and uninstaller, no admin rights needed. |
| | `vtt-<version>-windows-x86_64.zip` | Portable: unzip and run `vtt.exe`. |
| Linux (x86_64) | `vtt-<version>-x86_64.AppImage` | Single file: `chmod +x` and run. |
| | `vtt_<version>-1_amd64.deb` | Debian, Ubuntu: `sudo apt install ./vtt_*.deb` |
| | `vtt-<version>-1.x86_64.rpm` | Fedora, openSUSE: `sudo dnf install ./vtt-*.rpm` |
| | `vtt-<version>-linux-x86_64.tar.gz` | Binary, desktop entry and icons in a `/usr`-style tree. |

`SHA256SUMS` lists the checksums of all of them.

The macOS app and the Windows installer aren't signed by a registered developer yet, so the first launch shows a warning:

- **macOS:** right-click vtt in Applications and choose *Open* (once), or run `xattr -dr com.apple.quarantine /Applications/vtt.app`.
- **Windows:** SmartScreen says "Windows protected your PC": click *More info*, then *Run anyway*.

**Arch / CachyOS / Manjaro:** build and install a package from this checkout:

```sh
cd packaging/arch
makepkg -si
```

It packages the latest *committed* state and installs `vtt` to `/usr/bin` with a desktop entry and icon, so it shows up in your app launcher. If your Rust toolchain came from rustup's install script rather than pacman, use `makepkg -sid` so makepkg doesn't pull in the `rust` package as a build dependency. To update, pull (or commit) and run `makepkg -si` again. Remove it with `sudo pacman -R vtt-git`.

**From source, anywhere with Rust:** `scripts/install-local.sh` builds your working tree (uncommitted changes included, no sudo) and installs it for your user:

- Linux: `vtt` goes to `~/.cargo/bin`, with a desktop entry and icons in `~/.local/share`, so it shows up in your app launcher (`~/.cargo/bin` must be on the launcher's `PATH`). If the `vtt-git` package is also installed it wins on `PATH`, so remove it first.
- macOS: builds `vtt.app` and copies it to `/Applications`.

Or just `cargo install --path .` for the bare binary.

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

### Release packages

Each platform has a script that builds its packages into `dist/`:

| Script | Builds | Needs |
|---|---|---|
| `scripts/package-macos.sh` | `vtt.app`, `.dmg`, `.zip` (universal when both Apple targets are installed) | Xcode command line tools |
| `scripts/package-linux.sh` | `.tar.gz`, `.AppImage`, and `.deb` / `.rpm` | `cargo install cargo-deb cargo-generate-rpm` for the last two |
| `scripts/package-windows.ps1` | portable `.zip` and the installer | [Inno Setup 6](https://jrsoftware.org/isinfo.php) for the installer |

To publish a release, bump `version` in `Cargo.toml`, commit, and push a matching tag (`git tag v0.2.0 && git push origin v0.2.0`). The Release workflow builds every package on GitHub Actions and attaches them to a new GitHub release. The app icon lives in `packaging/icons/` (`render.sh` regenerates the PNGs and `.ico` from `vtt.svg`).

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

# Scale of the whole UI: sidebar, files panel, previews and terminals.
# Change it at runtime with Ctrl+Shift+= / Ctrl+Shift+- / Ctrl+Shift+0; the runtime
# zoom is remembered across restarts until you change this value.
ui_scale = 1.0

# Minimum contrast ratio between terminal text and its background (WCAG: 4.5 is AA, 7 is AAA).
# Text below it is lightened or darkened to reach it. 1.0 turns this off.
min_contrast = 1.0

# Turn off UI animations, smooth scrolling and cursor blinking.
reduce_motion = false

# Width in points of the accent border around the focused pane in a split (0 for none).
focus_border_width = 1.0

# What a terminal bell does: "badge" marks the tab in the sidebar, "flash" also flashes
# the pane, "none" ignores it.
bell = "badge"

[font]
# Font family; defaults to the system monospace font.
# family = "JetBrains Mono"
# Ctrl+= / Ctrl+- / Ctrl+0 zoom just the focused pane (terminal or preview).
size = 14.0
# Line height as a multiple of the font's own (e.g. 1.2 for more space between lines).
line_height = 1.0
# Extra space between characters, in points (may be negative).
letter_spacing = 0.0

[cursor]
# Shape when the program doesn't choose one: "block", "beam" or "underline".
style = "block"
# Blink the cursor (programs may also ask for blinking; never with reduce_motion).
blink = false
# Thickness multiplier for beam, underline and the unfocused outline.
thickness = 1.0

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
