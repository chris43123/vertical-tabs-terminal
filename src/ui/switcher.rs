//! Keyboard tab switcher / command palette (Ctrl+Shift+P): fuzzy-search tabs, commands and profiles.
//! Commands list their shortcut, so the palette doubles as a cheat sheet.

use eframe::egui::text::{LayoutJob, TextWrapping};
use eframe::egui::{
    self, Color32, CornerRadius, FontId, Id, Key, Modifiers, Order, Sense, Stroke, vec2,
};

use crate::app::App;
use crate::keybinds::{ACTIONS, Action};
use crate::session::TabId;

const ROW_HEIGHT: f32 = 28.0;

#[derive(Default)]
pub struct Switcher {
    query: String,
    selected: usize,
    /// Selection moved by keyboard this frame; scroll it into view.
    scroll: bool,
    initialized: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Entry {
    Tab(TabId),
    Command(Action),
    Profile(usize),
}

struct Row {
    entry: Entry,
    label: String,
    detail: String,
    score: i32,
}

impl App {
    pub(crate) fn switcher_ui(&mut self, ctx: &egui::Context) {
        let Some(mut sw) = self.switcher.take() else {
            return;
        };
        let rows = self.switcher_rows(&sw.query);

        let first_frame = !sw.initialized;
        if first_frame {
            sw.initialized = true;
            // Preselect the previously focused tab, so opening it and pressing Enter flips between two tabs.
            sw.selected = self
                .prev_focused
                .and_then(|p| rows.iter().position(|r| r.entry == Entry::Tab(p)))
                .unwrap_or(0);
            sw.scroll = true;
        }

        // Navigation keys are consumed before the text field sees them.
        let (mut close, mut run) = (false, false);
        ctx.input_mut(|i| {
            let down = i.consume_key(Modifiers::NONE, Key::ArrowDown)
                || i.consume_key(Modifiers::NONE, Key::Tab)
                || i.consume_key(Modifiers::CTRL, Key::N)
                || i.consume_key(Modifiers::CTRL, Key::J);
            let up = i.consume_key(Modifiers::NONE, Key::ArrowUp)
                || i.consume_key(Modifiers::SHIFT, Key::Tab)
                || i.consume_key(Modifiers::CTRL, Key::P)
                || i.consume_key(Modifiers::CTRL, Key::K);
            let n = rows.len().max(1);
            if down {
                sw.selected = (sw.selected + 1) % n;
                sw.scroll = true;
            }
            if up {
                sw.selected = (sw.selected + n - 1) % n;
                sw.scroll = true;
            }
            if i.consume_key(Modifiers::NONE, Key::PageDown) {
                sw.selected = (sw.selected + 8).min(n - 1);
                sw.scroll = true;
            }
            if i.consume_key(Modifiers::NONE, Key::PageUp) {
                sw.selected = sw.selected.saturating_sub(8);
                sw.scroll = true;
            }
            run = i.consume_key(Modifiers::NONE, Key::Enter);
            close = i.consume_key(Modifiers::NONE, Key::Escape);
        });
        // Pressing the palette shortcut again closes it (but not the press that just opened it).
        let events = if first_frame {
            Vec::new()
        } else {
            ctx.input(|i| i.events.clone())
        };
        for e in &events {
            if let egui::Event::Key {
                key,
                physical_key,
                pressed: true,
                modifiers,
                ..
            } = e
                && self.keybinds.lookup(*key, *physical_key, *modifiers)
                    == Some(Action::CommandPalette)
            {
                close = true;
            }
        }

        let screen = ctx.content_rect();
        let width = (screen.width() - 32.0).min(560.0);
        let pos = egui::pos2(screen.center().x - width / 2.0, screen.top() + 48.0);
        let c = self.chrome.clone();
        let mut clicked = None;
        let mut query_changed = false;

        let area = egui::Area::new(Id::new("switcher"))
            .fixed_pos(pos)
            .order(Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(c.raised(0.06))
                    .stroke(Stroke::new(1.0, c.raised(0.2)))
                    .corner_radius(CornerRadius::same(10))
                    .inner_margin(8)
                    .shadow(egui::Shadow {
                        offset: [0, 8],
                        blur: 28,
                        spread: 0,
                        color: Color32::from_black_alpha(140),
                    })
                    .show(ui, |ui| {
                        ui.set_width(width - 16.0);
                        let edit = ui.add(
                            egui::TextEdit::singleline(&mut sw.query)
                                .hint_text("Switch to a tab, or run a command…")
                                .font(FontId::proportional(15.0))
                                .desired_width(f32::INFINITY)
                                .frame(egui::Frame::NONE),
                        );
                        edit.request_focus();
                        if edit.changed() {
                            query_changed = true;
                        }
                        ui.separator();

                        if rows.is_empty() {
                            ui.weak("No matches");
                            return;
                        }
                        egui::ScrollArea::vertical()
                            .max_height(ROW_HEIGHT * 12.0)
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                for (i, row) in rows.iter().enumerate() {
                                    let (rect, resp) = ui.allocate_exact_size(
                                        vec2(ui.available_width(), ROW_HEIGHT),
                                        Sense::click(),
                                    );
                                    let selected = i == sw.selected;
                                    if selected {
                                        ui.painter().rect_filled(
                                            rect,
                                            CornerRadius::same(6),
                                            c.raised(0.16),
                                        );
                                        ui.painter().rect_filled(
                                            egui::Rect::from_min_size(
                                                rect.min + vec2(0.0, 6.0),
                                                vec2(3.0, rect.height() - 12.0),
                                            ),
                                            CornerRadius::same(2),
                                            c.accent,
                                        );
                                        if sw.scroll {
                                            resp.scroll_to_me(None);
                                        }
                                    } else if resp.hovered() {
                                        ui.painter().rect_filled(
                                            rect,
                                            CornerRadius::same(6),
                                            c.raised(0.1),
                                        );
                                    }
                                    let visuals = ui.visuals();
                                    let (kind, kind_color) = match row.entry {
                                        Entry::Tab(_) => ("tab", c.accent),
                                        Entry::Command(_) => ("cmd", c.purple),
                                        Entry::Profile(_) => ("new", c.green),
                                    };
                                    ui.painter().text(
                                        rect.left_center() + vec2(12.0, 0.0),
                                        egui::Align2::LEFT_CENTER,
                                        kind,
                                        FontId::monospace(11.0),
                                        kind_color,
                                    );
                                    let detail = ui.painter().layout_no_wrap(
                                        row.detail.clone(),
                                        FontId::monospace(11.5),
                                        visuals.weak_text_color(),
                                    );
                                    let detail_w = detail.size().x;
                                    ui.painter().galley(
                                        egui::pos2(
                                            rect.right() - 10.0 - detail_w,
                                            rect.center().y - detail.size().y / 2.0,
                                        ),
                                        detail,
                                        visuals.weak_text_color(),
                                    );
                                    let color = if selected {
                                        visuals.strong_text_color()
                                    } else {
                                        visuals.text_color()
                                    };
                                    let mut job = LayoutJob::simple_singleline(
                                        row.label.clone(),
                                        FontId::proportional(14.0),
                                        color,
                                    );
                                    job.wrap = TextWrapping::truncate_at_width(
                                        rect.width() - 64.0 - detail_w - 16.0,
                                    );
                                    let galley = ui.fonts_mut(|f| f.layout_job(job));
                                    ui.painter().galley(
                                        egui::pos2(
                                            rect.left() + 52.0,
                                            rect.center().y - galley.size().y / 2.0,
                                        ),
                                        galley,
                                        color,
                                    );
                                    if resp.clicked() {
                                        clicked = Some(row.entry);
                                    }
                                }
                            });
                    });
            });
        sw.scroll = false;

        // Clicking outside closes it.
        if ctx.input(|i| i.pointer.any_pressed())
            && ctx
                .input(|i| i.pointer.interact_pos())
                .is_some_and(|p| !area.response.rect.contains(p))
        {
            close = true;
        }
        if query_changed {
            sw.selected = 0;
        }

        let chosen = clicked.or_else(|| {
            run.then(|| rows.get(sw.selected).map(|r| r.entry))
                .flatten()
        });
        if let Some(entry) = chosen {
            match entry {
                Entry::Tab(id) => self.activate(id),
                Entry::Command(Action::CommandPalette) => {}
                Entry::Command(action) => {
                    self.run_action(action);
                }
                Entry::Profile(i) => {
                    self.new_tab(i, None);
                }
            }
            self.scroll_to_focused = true;
            return;
        }
        if !close {
            self.switcher = Some(sw);
        }
    }

    fn switcher_rows(&self, query: &str) -> Vec<Row> {
        let mut rows = Vec::new();
        for (i, &id) in self.ws.order.iter().enumerate() {
            let Some(tab) = self.tabs.get(&id) else {
                continue;
            };
            let mut detail = String::new();
            if tab.bell {
                detail.push_str("🔔 ");
            } else if tab.activity {
                detail.push_str("● ");
            }
            detail.push_str(&tab.profile.name);
            if i < 8 {
                detail.push_str(&format!(
                    "  {}",
                    self.keybinds
                        .label(Action::GotoTab(i as u8 + 1))
                        .unwrap_or_default()
                ));
            }
            let label = tab.title().to_string();
            let haystack = format!("{label} {}", tab.profile.name);
            if let Some(score) = fuzzy_score(query, &haystack) {
                rows.push(Row {
                    entry: Entry::Tab(id),
                    label,
                    detail,
                    score,
                });
            }
        }
        for &(_, label, action) in ACTIONS {
            if action == Action::CommandPalette {
                continue;
            }
            if let Some(score) = fuzzy_score(query, label) {
                let detail = self.keybinds.label(action).unwrap_or_default();
                rows.push(Row {
                    entry: Entry::Command(action),
                    label: label.to_string(),
                    detail,
                    score,
                });
            }
        }
        for (i, p) in self.profiles.iter().enumerate() {
            let label = format!("New tab: {}", p.name);
            if let Some(score) = fuzzy_score(query, &label) {
                rows.push(Row {
                    entry: Entry::Profile(i),
                    label,
                    detail: p.command.clone(),
                    score,
                });
            }
        }
        // Stable sort: equal scores keep tabs → commands → profiles order.
        rows.sort_by_key(|r| std::cmp::Reverse(r.score));
        rows
    }
}

/// Case-insensitive subsequence match. Higher is better; `None` if not all chars match.
/// Rewards matches at word starts and consecutive runs, penalises gaps.
fn fuzzy_score(query: &str, text: &str) -> Option<i32> {
    let query: Vec<char> = query
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect();
    if query.is_empty() {
        return Some(0);
    }
    let chars: Vec<char> = text.chars().collect();
    let mut qi = 0;
    let mut score = 0;
    let mut last: Option<usize> = None;
    for (i, c) in chars.iter().enumerate() {
        if qi == query.len() {
            break;
        }
        if c.to_lowercase().eq(std::iter::once(query[qi])) {
            let word_start = i == 0 || !chars[i - 1].is_alphanumeric();
            score += 10;
            if word_start {
                score += 8;
            }
            match last {
                Some(l) if l + 1 == i => score += 6,
                Some(l) => score -= ((i - l - 1) as i32).min(5),
                None => score -= (i as i32).min(10),
            }
            last = Some(i);
            qi += 1;
        }
    }
    (qi == query.len()).then_some(score)
}

#[cfg(test)]
mod tests {
    use super::fuzzy_score;

    #[test]
    fn fuzzy_matching() {
        assert_eq!(fuzzy_score("", "anything"), Some(0));
        assert!(fuzzy_score("xyz", "claude").is_none());
        assert!(fuzzy_score("cl", "claude · ~/proj").is_some());
        // Word starts and contiguous runs beat scattered matches.
        assert!(
            fuzzy_score("nt", "New tab").unwrap() > fuzzy_score("nt", "Go to next tab").unwrap()
        );
        assert!(
            fuzzy_score("clo", "Close tab").unwrap()
                > fuzzy_score("clo", "Scroll down one").unwrap()
        );
        assert!(fuzzy_score("CLAUDE", "claude").is_some());
    }
}
