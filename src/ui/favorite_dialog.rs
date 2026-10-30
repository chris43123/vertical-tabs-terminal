//! The add/edit favorite dialog: name, path, profile and command. Saving checks the path
//! exists and isn't already a favorite and that the command is one line; a problem shows
//! inline and nothing is saved. Enter saves, Esc or a click outside cancels.

use std::path::{Path, PathBuf};

use eframe::egui::{self, CornerRadius, FontId, Id, Key, Modifiers, RichText, Stroke, vec2};

use crate::app::App;
use crate::favorites::{Draft, Favorite, check_command, default_name};

pub struct FavoriteDialog {
    /// The favorite being edited; `None` adds one.
    editing: Option<String>,
    name: String,
    path: String,
    /// `None`: the default profile.
    profile: Option<String>,
    command: String,
    error: Option<String>,
    /// Keys are only read from the frame after it opened.
    initialized: bool,
}

impl FavoriteDialog {
    pub fn add(dir: &Path) -> Self {
        Self {
            editing: None,
            name: default_name(dir),
            path: dir.display().to_string(),
            profile: None,
            command: String::new(),
            error: None,
            initialized: false,
        }
    }

    pub fn edit(fav: &Favorite) -> Self {
        Self {
            editing: Some(fav.id.clone()),
            name: fav.name.clone(),
            path: fav.path.display().to_string(),
            profile: fav.profile.clone(),
            command: fav.command.clone().unwrap_or_default(),
            error: None,
            initialized: false,
        }
    }

    fn draft(&self) -> Draft {
        Draft {
            name: self.name.clone(),
            path: PathBuf::from(self.path.trim()),
            profile: self.profile.clone(),
            command: Some(self.command.clone()).filter(|c| !c.trim().is_empty()),
        }
    }
}

impl App {
    pub(crate) fn favorite_dialog_ui(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.favorite_dialog.take() else {
            return;
        };
        let (mut save, mut cancel) = (false, false);
        // While the profile list is open, Enter and Esc belong to it.
        if dialog.initialized && !ctx.any_popup_open() {
            ctx.input_mut(|i| {
                save = i.consume_key(Modifiers::NONE, Key::Enter);
                cancel = i.consume_key(Modifiers::NONE, Key::Escape);
            });
        }
        let first_frame = !dialog.initialized;
        dialog.initialized = true;

        let c = self.chrome.clone();
        let profiles: Vec<String> = self.profiles.iter().map(|p| p.name.clone()).collect();
        let default_profile = profiles.first().cloned().unwrap_or_default();
        // A command problem shows as you type, before Save.
        let command_problem = check_command(&dialog.command).err();
        let frame = egui::Frame::new()
            .fill(c.raised(0.06))
            .stroke(Stroke::new(1.0, c.raised(0.2)))
            .corner_radius(CornerRadius::same(10))
            .inner_margin(16);
        let title = if dialog.editing.is_some() {
            "Edit favorite"
        } else {
            "Add directory to favorites"
        };
        let modal = egui::Modal::new(Id::new("favorite_dialog"))
            .frame(frame)
            .backdrop_color(c.bg.gamma_multiply(0.55))
            .show(ctx, |ui| {
                ui.set_width(420.0);
                ui.label(
                    RichText::new(title)
                        .font(FontId::proportional(15.0))
                        .color(c.fg)
                        .strong(),
                );
                ui.add_space(8.0);
                egui::Grid::new("favorite_fields")
                    .num_columns(2)
                    .spacing(vec2(10.0, 8.0))
                    .show(ui, |ui| {
                        ui.label("Name");
                        let name = ui.add(
                            egui::TextEdit::singleline(&mut dialog.name)
                                .hint_text("emoji welcome: 🚀 api")
                                .desired_width(f32::INFINITY),
                        );
                        if first_frame {
                            name.request_focus();
                        }
                        ui.end_row();

                        ui.label("Path");
                        ui.add(
                            egui::TextEdit::singleline(&mut dialog.path)
                                .font(egui::TextStyle::Monospace)
                                .desired_width(f32::INFINITY),
                        );
                        ui.end_row();

                        ui.label("Profile");
                        let shown = dialog
                            .profile
                            .clone()
                            .unwrap_or_else(|| format!("Default ({default_profile})"));
                        egui::ComboBox::from_id_salt("favorite_profile")
                            .selected_text(shown)
                            .width(ui.available_width())
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut dialog.profile,
                                    None,
                                    format!("Default ({default_profile})"),
                                );
                                for p in &profiles {
                                    ui.selectable_value(&mut dialog.profile, Some(p.clone()), p);
                                }
                            });
                        ui.end_row();

                        ui.label("Command");
                        ui.add(
                            egui::TextEdit::singleline(&mut dialog.command)
                                .hint_text("optional, e.g. claude or k9s")
                                .font(egui::TextStyle::Monospace)
                                .desired_width(f32::INFINITY),
                        );
                        ui.end_row();
                    });
                ui.add_space(4.0);
                ui.label(
                    RichText::new("The command is typed into the shell when the favorite opens.")
                        .color(crate::theme::mix(c.fg, c.bg, 0.4))
                        .small(),
                );
                if let Some(err) = dialog.error.as_ref().or(command_problem.as_ref()) {
                    ui.add_space(6.0);
                    ui.label(RichText::new(format!("⚠ {err}")).color(c.danger));
                }
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    let ok =
                        egui::Button::new(RichText::new("Save").color(c.readable_on(c.accent)))
                            .fill(c.accent)
                            .min_size(vec2(72.0, 0.0));
                    if ui.add(ok).clicked() {
                        save = true;
                    }
                    let cancel_button = egui::Button::new(RichText::new("Cancel").color(c.fg))
                        .fill(c.raised(0.14))
                        .min_size(vec2(72.0, 0.0));
                    if ui.add(cancel_button).clicked() {
                        cancel = true;
                    }
                });
            });
        if modal.backdrop_response.clicked() {
            cancel = true;
        }
        if cancel {
            return;
        }
        if save {
            let editing = dialog.editing.clone();
            match self.save_favorite(editing.as_deref(), dialog.draft()) {
                Ok(()) => return,
                Err(err) => dialog.error = Some(err),
            }
        }
        self.favorite_dialog = Some(dialog);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drafts_from_the_fields() {
        let mut d = FavoriteDialog::add(Path::new("/srv/api"));
        assert_eq!(d.name, "api");
        assert_eq!(d.path, "/srv/api");
        d.command = "  ".into();
        d.path = " /srv/api ".into();
        let draft = d.draft();
        assert_eq!(draft.command, None);
        assert_eq!(draft.path, PathBuf::from("/srv/api"));
        let fav = Favorite {
            id: "f2".into(),
            name: "🚀 api".into(),
            path: "/srv/api".into(),
            profile: Some("fish".into()),
            command: Some("claude".into()),
        };
        let e = FavoriteDialog::edit(&fav);
        assert_eq!(e.editing.as_deref(), Some("f2"));
        assert_eq!(e.draft().command.as_deref(), Some("claude"));
        assert_eq!(e.draft().profile.as_deref(), Some("fish"));
    }
}
