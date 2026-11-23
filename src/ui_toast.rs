//! Lightweight floating toast notifications and Scratch-style beginner tip banners
//! for `diypets-settings` and `diypets-creator` so every button click gives immediate feedback.

use std::time::{Duration, Instant};

use eframe::egui;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Success,
    Info,
    Warn,
}

#[derive(Debug, Clone)]
pub struct ToastItem {
    pub kind: ToastKind,
    pub message: String,
    pub created_at: Instant,
    pub duration: Duration,
}

#[derive(Debug, Default)]
pub struct ToastQueue {
    items: Vec<ToastItem>,
}

impl ToastQueue {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn push(&mut self, kind: ToastKind, message: impl Into<String>, duration_ms: u64) {
        let msg = message.into();
        // Keep at most 4 stacked toasts so the corner stays clean
        if self.items.len() >= 4 {
            self.items.remove(0);
        }
        self.items.push(ToastItem {
            kind,
            message: msg,
            created_at: Instant::now(),
            duration: Duration::from_millis(duration_ms.max(400)),
        });
    }

    pub fn success(&mut self, msg: impl Into<String>) {
        self.push(ToastKind::Success, msg, 2400);
    }

    pub fn info(&mut self, msg: impl Into<String>) {
        self.push(ToastKind::Info, msg, 2600);
    }

    pub fn warn(&mut self, msg: impl Into<String>) {
        self.push(ToastKind::Warn, msg, 3600);
    }

    pub fn prune(&mut self, now: Instant) {
        self.items
            .retain(|t| now.saturating_duration_since(t.created_at) < t.duration);
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Draw animated bottom-right toast stack and schedule repaint while active.
    pub fn show(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        self.prune(now);
        if self.items.is_empty() {
            return;
        }
        ctx.request_repaint_after(Duration::from_millis(50));

        egui::Area::new(egui::Id::new("diypets_toast_stack"))
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-14.0, -14.0))
            .order(egui::Order::Foreground)
            .interactable(false)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 6.0;
                    for item in &self.items {
                        let (accent, prefix) = match item.kind {
                            ToastKind::Success => (egui::Color32::from_rgb(122, 199, 76), "OK"),
                            ToastKind::Info => (egui::Color32::from_rgb(126, 232, 250), "TIP"),
                            ToastKind::Warn => (egui::Color32::from_rgb(255, 179, 71), "NOTE"),
                        };
                        egui::Frame::NONE
                            .fill(egui::Color32::from_rgba_unmultiplied(15, 22, 32, 240))
                            .stroke(egui::Stroke::new(1.5, accent))
                            .corner_radius(egui::CornerRadius::same(6))
                            .inner_margin(egui::Margin::symmetric(12, 8))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new(format!("[{prefix}]"))
                                            .strong()
                                            .color(accent)
                                            .size(12.0),
                                    );
                                    ui.label(
                                        egui::RichText::new(&item.message)
                                            .color(egui::Color32::from_rgb(235, 240, 245))
                                            .size(12.5),
                                    );
                                });
                            });
                    }
                });
            });
    }
}

/// Compact beginner-friendly tip banner (Scratch-style guidance without clutter).
pub fn tip_banner(ui: &mut egui::Ui, label: &str, body: &str) {
    let accent = egui::Color32::from_rgb(126, 232, 250);
    egui::Frame::NONE
        .fill(egui::Color32::from_rgba_unmultiplied(20, 34, 46, 200))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(56, 92, 112)))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(label).strong().color(accent).size(12.0));
                ui.label(
                    egui::RichText::new(body)
                        .color(egui::Color32::from_rgb(215, 225, 232))
                        .size(12.0),
                );
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toast_queue_caps_stack_and_expires_old_entries() {
        let mut q = ToastQueue::new();
        for i in 0..6 {
            q.push(ToastKind::Success, format!("Action {i}"), 500);
        }
        assert_eq!(q.len(), 4, "stack should cap at 4 visible toasts");

        let future = Instant::now() + Duration::from_secs(2);
        q.prune(future);
        assert!(q.is_empty(), "expired toasts must clear cleanly");
    }
}
