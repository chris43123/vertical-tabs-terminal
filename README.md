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

- **Favorites.** Pin the directories you return to above the tabs, each with an optional profile and command (`claude`, `k9s`, `npm run dev`). One click opens it, and from then on the favorite's row *is* that tab. Scripts and AI agents manage the same list with `vtt favorites` (see [Favorites](#favorites)).
- **Session restore.** Quit and relaunch, and your tabs come back: their order, folders, splits, names, directories and favorite links. Only the tabs on screen start a shell; the rest wait, dimmed, until you open them, so a big session costs nothing at launch (see [Session restore](#session-restore)).
- **Vertical tabs.** The sidebar can be collapsed to an icon strip, or hidden entirely (zen mode, `Ctrl+B`) and peeked by hovering the left window edge (`Ctrl+\` in zen mode switches the peek between tabs and files).
- **Folders.** Group tabs into named, colored folders that collapse, Zen-style (see [Folders](#folders)).
- **Files panel.** A file tree of the focused shell's directory sits between the tabs and the terminals, and follows you as you `cd` (see [Files and previews](#files-and-previews)).
- **File previews.** Click a file to open it in a pane next to the terminal: rendered Markdown, syntax-highlighted code and JSON, images. Copy the whole file in one click, select lines by their numbers, or copy a `path:12-40` reference for an agent.
- **Git at a glance.** The files panel shows the branch, commits ahead/behind, and changed files highlighted in green.
- **File search.** Fuzzy-find any file in the project from the files panel.
- **Reset tab.** Every tab remembers how it was launched (shell, directory, name). "Reset tab" (`⌘⇧R` on macOS, or the palette and the tab's right-click menu) kills everything running in it, background jobs included, and starts a fresh shell in the same slot, split and folder, in the directory it was opened in. A wedged REPL, a hung `ssh` or a runaway build is one click from a clean prompt. It asks first if a program is running (`confirm_reset`; on Windows vtt can't tell, so it never asks). On Linux/macOS, processes that detach with `setsid` (daemons) leave the tab's session and survive by design.
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
| Reset tab (kill what runs in it, start it again in place) | — | `⌘⇧R` |
| Add directory to favorites | — | — |
| Open favorite 1–9 | — | — |

**Terminal**

| Action | Linux / Windows | macOS |
|---|---|---|
| Zoom focused pane in / out / reset | `Ctrl+=` / `Ctrl+-` / `Ctrl+0` | `⌘=` / `⌘-` / `⌘0` |
| Zoom whole UI in / out / reset | `Ctrl+Shift+=` / `Ctrl+Shift+-` / `Ctrl+Shift+0` | `⌘⇧=` / `⌘⇧-` / `⌘⇧0` |
| Scroll back | `Shift+PageUp` / `Shift+PageDown` | same |
| Open settings file | `Ctrl+,` | `⌘,` |
| Keyboard shortcuts: every shortcut and mouse gesture, click one to rebind it | `Ctrl+Shift+/` (`Ctrl+?`) | `⌘⇧/` (`⌘?`) |
| Copy / paste | `Ctrl+Shift+C` / `Ctrl+Shift+V`, plus `Ctrl+V` | `⌘C` / `⌘V` |
| Select all / copy in a focused preview | `Ctrl+A` / `Ctrl+C` or `Ctrl+Shift+C` | `⌘A` / `⌘C` |

`Ctrl+C` copies when text is selected; otherwise it sends an interrupt as usual. Set `copy_on_select = true` to copy as soon as you finish selecting with the mouse. Pane-focus and next/previous-tab keys pass through to the shell when there is nothing to move to (a single tab, or no pane in that direction).

**The palette** (`Ctrl+Shift+P`) fuzzy-searches your tabs, every command (each shown with its shortcut, so it doubles as a cheat sheet), "new tab with profile X" and "Open favorite: X".
- It opens with your previously focused tab selected, so `Ctrl+Shift+P`, `Enter` flips between two tabs.
- Move with `Up`/`Down`, `Tab`/`Shift+Tab` or `Ctrl+N`/`Ctrl+P`; `Enter` picks and `Esc` closes.

Every shortcut can be changed in the `[keybindings]` section of the config (see below). Setting an action replaces all of its defaults. Use a list for several chords, and `[]` to unbind it. Modifiers are `ctrl`, `shift`, `alt` and `cmd` (macOS only). Keys are letters, digits, `f1`–`f24`, `tab`, `enter`, `space`, `pageup`, `pagedown`, `home`, `end`, `left`/`right`/`up`/`down`, `plus`, `minus`, `equals`, `[`, `]` and similar.

Actions: `new_tab`, `close_tab`, `reopen_closed_tab`, `duplicate_tab`, `rename_tab`, `reset_tab`, `split_right`, `split_down`, `minimize_pane`, `toggle_sidebar`, `toggle_side_area`, `next_tab`, `prev_tab`, `move_tab_up`, `move_tab_down`, `goto_tab_1` … `goto_tab_9`, `last_tab`, `next_activity`, `command_palette`, `toggle_files`, `search_files`, `new_group`, `open_settings`, `focus_left`, `focus_right`, `focus_up`, `focus_down`, `zoom_in`, `zoom_out`, `zoom_reset`, `ui_zoom_in`, `ui_zoom_out`, `ui_zoom_reset`, `scroll_page_up`, `scroll_page_down`, `show_help`, `add_favorite`, `open_favorite_1` … `open_favorite_9`.

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
- **Copying.** **Copy** in the toolbar copies the selection, or the whole file when nothing is selected (for rendered markdown, its source). A short note at the bottom of the pane confirms what was copied ("Copied 3,000 lines"), or says so if the clipboard couldn't be written. In code and text (and markdown source):
  - Drag to select, `Shift+click` to extend, double-click a word, triple-click a line. The selection stays put however far you scroll, and dragging near or past the top or bottom edge scrolls, faster the further out you go (sideways too when lines don't wrap).
  - **Line numbers select whole lines:** click one to select that line, `Shift+click` another to select everything in between (lines 1 to 3000 in two clicks), or drag down the numbers.
  - `Ctrl+A` / `Ctrl+C` (`⌘A` / `⌘C`) select all and copy while the preview is focused; `Esc` clears the selection.
  - **Right-click** for Copy, Copy all and **Copy reference**, which copies `path:12-40` (or `path:12`) for the selected lines, the form editors and coding agents understand.
  - Rendered markdown keeps its own text selection, and dragging it near the edge scrolls too. To copy a long passage, use **Source** and the line numbers, or **Copy** for the whole file.
- **Images** (PNG, JPEG, GIF, WebP) are shown fitted to the pane.
- Binary and very large files show their size and a button to open them with the default app.

## Folders

Folders group tabs in the sidebar, like folders in Zen Browser.

- Right-click a tab and pick **New folder with tab** (or run it from the palette), then type a name.
- Click a folder's header to collapse or expand it. A collapsed folder still shows the tab you're on, plus how many tabs it holds and a dot if one of them has new output.
- To add a tab to a folder, drag it onto the folder's header or between its tabs, or use **Move to folder**. Drag a tab out (or use **Remove from folder**) to take it out.
- New tabs opened from a tab in a folder join that folder. A split view always moves as a unit.
- Right-click a folder's header to rename it, change its color, open a new tab in it, ungroup it, or close all its tabs.

## Favorites

Favorites are directories pinned above the tabs in the sidebar. Each has a name (emoji welcome), a path, and optionally a profile and a command that is typed into the shell when it opens, so aliases and your `PATH` work and quitting the command leaves you at a prompt in that directory.

- **Click** a favorite to open its tab. The tab doesn't get a second row among the other tabs: the favorite's row is the tab. It highlights when focused, shows the unread and bell dots, and closes with × (on hover) or a middle click; the favorite stays. A click while it's open focuses it.
- **Splits stay with it.** Split a favorite's tab ("Split right with new tab" in its menu, or drag a tab onto its pane) and the other panes are listed under the favorite, not among the tabs. Next/previous tab and "go to tab N" count the favorites' tabs first, in the sidebar's order.
- The same directory can be a favorite more than once (a plain shell and `k9s` in one place, say). Favorites are told apart by id, so each opens and focuses its own tab, even with the same name or path.
- Each favorite has a **tile** with the first letter or emoji of its name (a leading emoji isn't repeated in the name next to it). The **dot** on the tile's corner is green when its tab is open, a ring when it has none, and red when its shell exited on its own; click it again to start a new one. The unread and bell dots sit on the tile's other corner.
- **↻** (on hover) resets its tab: everything in it is killed and it starts again from the favorite as it is *now*, so a command you changed since takes effect. Reset tab on a favorite's tab does the same. With no tab open, the hover button is ▶ and opens one.
- **Drag** to reorder. **Right-click** for Reset, Split right, Minimise from split, Reveal in files panel, Rename / edit…, Close tab and Remove favorite (with no tab open: Open, Open in split, Reveal, Rename / edit… and Remove). Open in split into another favorite's tab opens a plain tab started like this favorite (directory, profile, command) in that favorite's split; this favorite's own row stays closed. Double-click opens the edit dialog.
- The **Favorites** header has a **+** on hover to add one. Drag the line under the section to make it taller or shorter (double-click it to go back to the default); the height is remembered. Past that height the section scrolls, so the tabs always keep room.
- A favorite whose directory is gone is dimmed, and clicking it only says so. vtt checks when it loads the list, when the list changes and when you click; it never polls.
- With the sidebar collapsed, a favorite shows just its tile.
- **Add one** with "Add directory to favorites" in a tab's right-click menu (its current directory), in a folder's right-click menu in the files panel, or from the palette. The dialog asks for the name, path, profile and command. It won't save a path that doesn't exist, or a command with more than one line.

### From the command line

`vtt favorites` edits the same list, whether or not vtt is running. A running vtt shows the change within a second.

```sh
vtt favorites add                                    # the current directory, named after it
vtt favorites add ~/src/api --name "🚀 api" --command claude
vtt favorites add ~/infra --profile fish --command k9s
vtt favorites list                                   # position, id, name, path, profile, command
vtt favorites list --json                            # [{id, name, path, profile, command, position, exists}]
vtt favorites set api command "npm run dev"          # fields: name, path, profile, command
vtt favorites unset 2 command                        # profile and command can be unset
vtt favorites move f3 1                              # to the top
vtt favorites rm "🚀 api"
vtt favorites path                                   # where the file lives
```

`<fav>` is a position (`1`), an id (`f3`) or an exact name. Ids are stable and never reused, so scripts should prefer them; a name shared by several favorites is an error that lists their ids. Exit codes: 0 ok, 1 error (the message says what to fix, and the file is left as it was), 2 bad usage (prints the usage).

The list lives in its own file, per machine, not in the config:

- Linux: `~/.local/share/vtt/favorites.toml`
- macOS: `~/Library/Application Support/vtt/favorites.toml` (next to `config.toml`, but still a separate file)
- Windows: `%APPDATA%\vtt\favorites.toml`

It's plain TOML (`[[favorite]]` tables), so you can edit it by hand too. vtt and the CLI write it atomically and re-read it before every change, so neither overwrites the other's edits. A favorite's command is typed into a shell, so anything that can write this file can run commands as you, just like your shell's rc files. vtt only types one line: a hand-edited command with a line break or another control character (an escape sequence, say) is ignored with a warning, and **dropped from the file the next time vtt or `vtt favorites` writes it**. Control characters in a hand-edited name are replaced the same way.

## Session restore

vtt saves the open tabs a couple of seconds after the layout changes and when it quits, and brings them back at the next launch (`restore_session = true`, the default):

- Tab order, folders (name, color, collapsed), split views with their proportions and names, the focused pane and the view on screen, tab names, each tab's last directory, the command it was opened with, and which favorite it belongs to.
- Only the panes on screen start their shell at launch. Every other tab comes back **dormant**: dimmed in the sidebar, no process behind it, and a favorite's dot shows no session. Showing it any way (a click, the switcher, next/previous tab, a favorite, dropping it into a split) starts its shell in its saved directory and types its command. Reset on a dormant tab just starts it; closing one just removes it.
- Not restored: scrollback, running programs (a restored tab starts a fresh shell), file previews and the settings tab.
- If a tab's profile no longer exists, it opens the default profile without its command, with a warning. A directory that's gone becomes the profile's directory, else home. A link to a removed favorite is dropped. A session file that can't be read is moved to `session.toml.bad` with a warning, and vtt starts fresh.
- With `restore_session = false`, vtt neither reads nor writes the session file (an existing one is left alone).

The file is `session.toml` next to `favorites.toml` (see [Favorites](#favorites) for the folder). With several vtt windows open, the last one to save wins.

### From the command line

`vtt session` reads the saved session, for scripts and agents:

```sh
vtt session show          # one line per tab: position, * for the focused one, title, profile,
                          # directory, command, folder, favorite, split
vtt session show --json   # {"active": n, "tabs": [{position, active, title, profile, dir,
                          #   command, folder, favorite, split}], "folders": [{position, name, color, collapsed}]}
vtt session clear         # forget it (quit vtt first: a running vtt saves it again)
vtt session path          # where the file lives
```

A running vtt doesn't watch the file: it mirrors what's open, and its next save overwrites a change made from outside. With no file, `show` prints an empty session. Exit codes: 0 ok, 1 error (an unreadable file, with what to do), 2 bad usage (prints the usage).

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
- macOS: builds `vtt.app` and copies it to `/Applications`. This doesn't put a `vtt` command on your `PATH`; to use the [command line](#from-the-command-line), link the app's binary into a directory that is, once (later installs update it through the link):

  ```sh
  ln -sf /Applications/vtt.app/Contents/MacOS/vtt ~/.local/bin/vtt
  ```

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

### From the command line

`vtt config` reads and changes the config file without opening it, which is handy for scripts, dotfiles and AI agents. Keys in tables are dotted (`font.size`, `cursor.style`, `keybindings.new_tab`).

```sh
vtt config get font.size                         # 14.0
vtt config set font.size 16                      # font.size = 16.0
vtt config set editor "code --wait"
vtt config set keybindings.new_tab ctrl+alt+t f2  # one or more shortcuts
vtt config set keybindings.close_tab             # no shortcuts: unbind it
vtt config unset font.size                       # back to the default
vtt config check                                 # validate (exit code 1 on problems)
vtt config reference                             # every key, default and allowed value
vtt config path                                  # where the file lives
```

`vtt config` with no subcommand (or `vtt config --help`) prints this list. Any other arguments, including `vtt --help`, are ignored and open the app.

`set` creates the file from the example if needed, keeps your comments and layout, and refuses values of the wrong type, out of range or otherwise invalid (an unknown theme, a bad shortcut), leaving the file untouched. A running vtt picks the change up live. Profiles and colors are edited in the file itself.

The full reference is also written as `REFERENCE.md` next to the config file.

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

# Ask before "Reset tab" kills a program running in the tab (vim, a build, ssh...).
# Resetting a tab that is just at its prompt never asks. Windows never asks.
confirm_reset = true

# Bring back the tabs from last time at launch: order, folders, splits, names and
# directories. Only the tabs on screen start a shell; the others start when you open them.
# Running programs and scrollback aren't restored. Off: no session file is read or written.
restore_session = true

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
# foreground, background, cursor, selection, selection_foreground, ansi = [16 colors],
# colorN (any index 0-255), and [colors.ui] accent / sidebar.
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
# reset_tab = "ctrl+shift+r"               # Linux/Windows: unbound by default (macOS: cmd+shift+r)
# goto_tab_1 = "alt+1"                     # Linux/Windows: most actions are unbound
# add_favorite = "ctrl+shift+f2"           # unbound by default, on every OS
# open_favorite_1 = "ctrl+f1"              # open_favorite_1 … _9: unbound by default

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
  selection_foreground = "#c0caf5"   # optional; see below
  ansi = ["#15161e", "#f7768e", "#9ece6a", "#e0af68", "#7aa2f7", "#bb9af7", "#7dcfff", "#a9b1d6",
          "#414868", "#f7768e", "#9ece6a", "#e0af68", "#7aa2f7", "#bb9af7", "#7dcfff", "#c0caf5"]
  color232 = "#101010"   # any palette index
  [ui]
  accent = "#7aa2f7"     # optional; defaults to color4
  sidebar = "#16161e"    # optional; defaults to a shade of the background
  ```
- **kitty format** (any other extension). `background`, `foreground`, `cursor`, `selection_background`, `selection_foreground`, `colorN` and `active_border_color` (used as the accent) are read; everything else is ignored. Existing kitty themes and generated theme files therefore work unchanged.

The sidebar, pane headers and palette derive their colors from the theme, so light themes work too.

Selected text stays readable, in terminals and previews alike. With `selection_foreground` in the theme, all selected text uses it (as in kitty). Without it, text keeps its color when that reads well on the selection, and otherwise switches to the theme's foreground or background, whichever contrasts more. `min_contrast` applies on top.

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
selection_foreground = "{{colors.on_secondary_container.default.hex}}"
[ui]
accent = "{{colors.primary.default.hex}}"
sidebar = "{{colors.surface_container_low.default.hex}}"
```

This only sets the base and UI colors; the 16 ANSI colors stay from the default theme unless you add `colorN` lines too. Material You doesn't define ANSI colors, which is why wallpaper setups usually generate a separate terminal palette.

## License

MIT
