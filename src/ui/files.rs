//! The files panel between the tab sidebar and the panes: a tree of the focused shell's cwd
//! that follows it as you `cd`. Clicking a file previews it next to the terminal; folders
//! expand in place, and a double-click `cd`s the shell into them.

use std::path::{Path, PathBuf};

use eframe::egui::text::{LayoutJob, TextWrapping};
use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, RichText, Sense, Ui, pos2, vec2};

use crate::app::{App, Content};
use crate::files::{self, Row};
use crate::keybinds::Action as Shortcut;
use crate::theme::mix;

const ROW_HEIGHT: f32 = 22.0;
const INDENT: f32 = 14.0;

/// Payload for dragging a file out of the tree (dropped on a terminal, it types the path).
#[derive(Clone, Debug)]
pub struct FileDrag(pub PathBuf);

enum Action {
    Toggle(PathBuf),
    Preview(PathBuf),
    Browse(PathBuf),
    Cd(PathBuf),
    NewTabIn(PathBuf),
    InsertPath(PathBuf),
    CopyPath(PathBuf),
    OpenExternal(PathBuf),
    ToggleHidden,
    CollapseAll,
    Close,
}

impl App {
    pub(crate) fn files_panel(&mut self, ui: &mut Ui) {
        self.sync_files_root();
        let c = self.chrome.clone();
        let fill = mix(c.sidebar, c.bg, 0.45);
        let mut actions = Vec::new();
        egui::Panel::left("files")
            .resizable(true)
            .default_size(self.config.files.width)
            .size_range(160.0..=720.0)
            .frame(egui::Frame::new().fill(fill).inner_margin(6))
            .show(ui, |ui| self.files_contents(ui, &mut actions));
        for action in actions {
            self.apply_file_action(action);
        }
    }

    /// Follow the focused shell when focus moves to another tab or its cwd changed.
    fn sync_files_root(&mut self) {
        let focused = self.ws.focused().and_then(|f| self.tabs.get(&f));
        let cwd = focused
            .filter(|t| matches!(t.content, Content::Term(_)))
            .and_then(|t| t.cwd.clone());
        match cwd {
            Some(cwd) if self.files_followed.as_ref() != Some(&cwd) => {
                self.files.set_root(cwd.clone());
                self.files_followed = Some(cwd);
            }
            None if self.files.root().is_none() => {
                // Fresh tab without a polled cwd yet, or a preview: ask directly.
                let dir = self
                    .ws
                    .focused()
                    .and_then(|f| self.tabs.get(&f))
                    .and_then(|t| t.dir().map(Path::to_path_buf))
                    .or_else(dirs::home_dir);
                if let Some(dir) = dir {
                    self.files.set_root(dir);
                }
            }
            _ => {}
        }
    }

    fn files_contents(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        ui.spacing_mut().item_spacing = vec2(4.0, 2.0);
        let c = self.chrome.clone();
        let Some(root) = self.files.root().map(Path::to_path_buf) else {
            ui.weak("No folder");
            return;
        };

        // Header: folder name and actions, then the full path.
        let name = root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| root.to_string_lossy().into_owned());
        ui.horizontal(|ui| {
            ui.add(
                egui::Label::new(RichText::new(&name).strong())
                    .truncate()
                    .sense(Sense::hover()),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let close_tip = format!("Hide files{}", self.keybinds.hint(Shortcut::ToggleFiles));
                if ui.small_button("×").on_hover_text(close_tip).clicked() {
                    actions.push(Action::Close);
                }
                if ui
                    .small_button("⊟")
                    .on_hover_text("Collapse all folders")
                    .clicked()
                {
                    actions.push(Action::CollapseAll);
                }
                let hidden = ui
                    .selectable_label(self.files.show_hidden, ".*")
                    .on_hover_text("Show hidden files");
                if hidden.clicked() {
                    actions.push(Action::ToggleHidden);
                }
                if let Some(parent) = root.parent()
                    && ui
                        .small_button("⬆")
                        .on_hover_text("Parent folder (until the shell changes directory)")
                        .clicked()
                {
                    actions.push(Action::Browse(parent.to_path_buf()));
                }
            });
        });
        let mut job = LayoutJob::simple_singleline(
            crate::ui::display_path(&root),
            FontId::proportional(11.5),
            mix(c.fg, fill_of(ui), 0.45),
        );
        job.wrap = TextWrapping::truncate_at_width(ui.available_width());
        ui.label(job).on_hover_text(root.to_string_lossy());
        ui.add_space(2.0);
        ui.separator();

        let rows = self.files.rows();
        let selected = self.visible_preview_path();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .id_salt("files_tree")
            .show_rows(ui, ROW_HEIGHT, rows.len(), |ui, range| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for row in &rows[range] {
                    self.file_row(ui, row, selected.as_deref(), actions);
                }
            });
    }

    fn file_row(&self, ui: &mut Ui, row: &Row, selected: Option<&Path>, actions: &mut Vec<Action>) {
        let c = &self.chrome;
        let (rect, resp) = ui.allocate_exact_size(
            vec2(ui.available_width(), ROW_HEIGHT),
            Sense::click_and_drag(),
        );
        let painter = ui.painter_at(rect);
        let (depth, entry, expanded) = match row {
            Row::Entry {
                entry,
                depth,
                expanded,
            } => (*depth, Some(entry), *expanded),
            Row::More { depth, .. } | Row::Error { depth, .. } => (*depth, None, false),
        };
        let x = rect.left() + 6.0 + depth as f32 * INDENT;
        let fill = fill_of(ui);
        let dim = mix(c.fg, fill, 0.5);

        let Some(entry) = entry else {
            let text = match row {
                Row::More { count, .. } => format!("… {count} more"),
                Row::Error { message, .. } => format!("⚠ {message}"),
                Row::Entry { .. } => unreachable!(),
            };
            painter.text(
                pos2(x + 14.0, rect.center().y),
                Align2::LEFT_CENTER,
                text,
                FontId::proportional(12.0),
                dim,
            );
            return;
        };

        let is_selected = selected == Some(entry.path.as_path());
        let bg = if is_selected {
            mix(c.accent, fill, 0.72)
        } else if resp.hovered() {
            mix(fill, c.fg, 0.07)
        } else {
            Color32::TRANSPARENT
        };
        painter.rect_filled(rect, CornerRadius::same(4), bg);

        // Indent guides.
        for d in 0..depth {
            let gx = rect.left() + 6.0 + d as f32 * INDENT + 4.0;
            painter.vline(
                gx,
                rect.y_range(),
                egui::Stroke::new(1.0, mix(fill, c.fg, 0.1)),
            );
        }
        if entry.is_dir {
            painter.text(
                pos2(x + 4.0, rect.center().y),
                Align2::CENTER_CENTER,
                if expanded { "⏷" } else { "⏵" },
                FontId::proportional(12.0),
                dim,
            );
        }
        let color = if entry.is_dir {
            mix(c.accent, c.fg, 0.35)
        } else {
            file_color(&entry.name, c)
        };
        let color = if entry.hidden {
            mix(color, fill, 0.45)
        } else {
            color
        };
        let mut job =
            LayoutJob::simple_singleline(entry.name.clone(), FontId::proportional(13.0), color);
        job.wrap = TextWrapping::truncate_at_width(rect.right() - x - 18.0);
        let galley = ui.fonts_mut(|f| f.layout_job(job));
        painter.galley(
            pos2(x + 14.0, rect.center().y - galley.size().y / 2.0),
            galley,
            color,
        );

        let path = entry.path.clone();
        if resp.double_clicked() && entry.is_dir {
            // Undo the expand/collapse of the first click, then cd.
            actions.push(Action::Toggle(path.clone()));
            actions.push(Action::Cd(path.clone()));
        } else if resp.clicked() {
            actions.push(if entry.is_dir {
                Action::Toggle(path.clone())
            } else {
                Action::Preview(path.clone())
            });
        }
        if resp.drag_started() {
            egui::DragAndDrop::set_payload(ui.ctx(), FileDrag(path.clone()));
        }
        let is_dir = entry.is_dir;
        resp.on_hover_text_at_pointer(path.to_string_lossy())
            .context_menu(|ui| {
                let mut item = |ui: &mut Ui, text: &str, action: Action| {
                    if ui.button(text).clicked() {
                        actions.push(action);
                        ui.close();
                    }
                };
                if is_dir {
                    item(ui, "cd into folder", Action::Cd(path.clone()));
                    item(ui, "New tab here", Action::NewTabIn(path.clone()));
                    item(ui, "Browse here", Action::Browse(path.clone()));
                } else {
                    item(ui, "Preview", Action::Preview(path.clone()));
                }
                ui.separator();
                item(
                    ui,
                    "Insert path into terminal",
                    Action::InsertPath(path.clone()),
                );
                item(ui, "Copy path", Action::CopyPath(path.clone()));
                item(
                    ui,
                    "Open with default app",
                    Action::OpenExternal(path.clone()),
                );
            });
    }

    /// The file shown by a preview pane in the active view, highlighted in the tree.
    fn visible_preview_path(&self) -> Option<PathBuf> {
        self.ws.visible().iter().find_map(|id| {
            self.tabs
                .get(id)
                .and_then(|t| t.preview())
                .map(|p| p.path.clone())
        })
    }

    fn apply_file_action(&mut self, action: Action) {
        match action {
            Action::Toggle(p) => self.files.toggle(&p),
            Action::Preview(p) => self.open_preview(p),
            Action::Browse(p) => self.browse_files(p),
            Action::Cd(p) => self.cd_focused(&p),
            Action::NewTabIn(p) => self.new_tab_in(p),
            Action::InsertPath(p) => self.insert_path(&p),
            Action::CopyPath(p) => self.set_clipboard(p.to_string_lossy().into_owned()),
            Action::OpenExternal(p) => {
                if let Err(err) = files::open_with_default_app(&p) {
                    crate::diag::warn(err);
                }
            }
            Action::ToggleHidden => self.files.show_hidden = !self.files.show_hidden,
            Action::CollapseAll => self.files.collapse_all(),
            Action::Close => self.files_open = false,
        }
    }
}

/// Background the current ui paints on (for blending dim colors).
fn fill_of(ui: &Ui) -> Color32 {
    ui.visuals().panel_fill
}

/// A hint of color by file type, from the theme's ANSI palette.
fn file_color(name: &str, c: &crate::theme::UiColors) -> Color32 {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase());
    match ext.as_deref() {
        Some("md" | "markdown" | "txt" | "rst") => mix(c.fg, c.accent, 0.25),
        Some("json" | "toml" | "yaml" | "yml" | "ini" | "conf" | "lock") => mix(c.fg, c.bell, 0.45),
        Some("png" | "jpg" | "jpeg" | "gif" | "webp" | "svg") => mix(c.fg, c.purple, 0.5),
        Some("sh" | "bash" | "zsh" | "fish" | "ps1") => mix(c.fg, c.green, 0.5),
        _ => c.fg,
    }
}
