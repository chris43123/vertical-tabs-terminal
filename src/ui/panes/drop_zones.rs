//! Drag-and-drop onto panes: the split/swap zones shown while dragging a tab or a file from
//! the files panel, and what a drop does.

use eframe::egui::{self, Align2, CornerRadius, FontId, Id, Rect, Stroke, StrokeKind, Ui, pos2};

use crate::app::{App, TabDrag};
use crate::workspace::{Drop, Edge, TabId};

use super::PaneAction;

impl App {
    /// A file or folder dragged from the files panel onto a pane: an edge opens it in a new
    /// split there (a preview, or a terminal for a folder); the center types its path into a
    /// terminal, or shows the file in a preview.
    pub(super) fn file_drop(&self, ui: &Ui, id: TabId, rect: Rect, actions: &mut Vec<PaneAction>) {
        let Some(drag) = egui::DragAndDrop::payload::<crate::ui::FileDrag>(ui.ctx()) else {
            return;
        };
        let Some(pos) = ui.ctx().input(|i| i.pointer.hover_pos()) else {
            return;
        };
        if !rect.contains(pos) {
            return;
        }
        let drop = drop_for(rect, pos);
        let is_preview = self.tabs.get(&id).is_some_and(|t| t.preview().is_some());
        let label = match drop {
            Drop::Edge(_) if drag.0.is_dir() => "Terminal here",
            Drop::Edge(_) => "Split",
            Drop::Center if is_preview && drag.0.is_dir() => return,
            Drop::Center if is_preview => "Show here",
            Drop::Center => "Insert path",
        };
        self.paint_drop_target(ui, rect, drop, label);
        if ui.ctx().input(|i| i.pointer.any_released()) {
            actions.push(PaneAction::DropPath(id, drag.0.clone(), drop));
        }
    }

    /// Show split/swap targets while a tab is dragged over this pane; apply on release.
    pub(super) fn drop_zone(&self, ui: &Ui, id: TabId, rect: Rect, actions: &mut Vec<PaneAction>) {
        let Some(drag) = egui::DragAndDrop::payload::<TabDrag>(ui.ctx()) else {
            return;
        };
        let Some(pos) = ui.ctx().input(|i| i.pointer.hover_pos()) else {
            return;
        };
        if !rect.contains(pos) || drag.0 == id {
            return;
        }
        let drop = drop_for(rect, pos);
        let label = if drop == Drop::Center {
            "Swap"
        } else {
            "Split"
        };
        self.paint_drop_target(ui, rect, drop, label);

        if ui.ctx().input(|i| i.pointer.any_released()) {
            actions.push(PaneAction::Drop(drag.0, id, drop));
        }
    }

    /// Highlight the half (edge) or whole (center) of pane `rect` a drop would land on.
    fn paint_drop_target(&self, ui: &Ui, rect: Rect, drop: Drop, label: &str) {
        let target = match drop {
            Drop::Edge(Edge::Left) => {
                Rect::from_min_max(rect.min, pos2(rect.center().x, rect.bottom()))
            }
            Drop::Edge(Edge::Right) => {
                Rect::from_min_max(pos2(rect.center().x, rect.top()), rect.max)
            }
            Drop::Edge(Edge::Top) => {
                Rect::from_min_max(rect.min, pos2(rect.right(), rect.center().y))
            }
            Drop::Edge(Edge::Bottom) => {
                Rect::from_min_max(pos2(rect.left(), rect.center().y), rect.max)
            }
            Drop::Center => rect,
        };
        let painter = ui.ctx().layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            Id::new("drop_zone"),
        ));
        painter.rect_filled(
            target.shrink(4.0),
            CornerRadius::same(8),
            self.chrome.accent.gamma_multiply(0.22),
        );
        painter.rect_stroke(
            target.shrink(4.0),
            CornerRadius::same(8),
            Stroke::new(2.0, self.chrome.accent),
            StrokeKind::Inside,
        );
        painter.text(
            target.center(),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(16.0),
            self.chrome.fg,
        );
    }
}

/// Which drop target the pointer is over: outer 25% bands split, the middle swaps.
fn drop_for(rect: Rect, pos: egui::Pos2) -> Drop {
    let x = (pos.x - rect.left()) / rect.width();
    let y = (pos.y - rect.top()) / rect.height();
    let edges = [
        (x, Edge::Left),
        (1.0 - x, Edge::Right),
        (y, Edge::Top),
        (1.0 - y, Edge::Bottom),
    ];
    let (dist, edge) = edges
        .into_iter()
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .unwrap();
    if dist < 0.25 {
        Drop::Edge(edge)
    } else {
        Drop::Center
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drop_targets() {
        let r = Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 100.0));
        assert_eq!(drop_for(r, pos2(5.0, 50.0)), Drop::Edge(Edge::Left));
        assert_eq!(drop_for(r, pos2(95.0, 50.0)), Drop::Edge(Edge::Right));
        assert_eq!(drop_for(r, pos2(50.0, 90.0)), Drop::Edge(Edge::Bottom));
        assert_eq!(drop_for(r, pos2(50.0, 50.0)), Drop::Center);
    }
}
