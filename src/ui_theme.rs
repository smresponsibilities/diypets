//! Evidence-backed dark UI theme and ergonomic widget helpers for `diypets-creator`
//! and `diypets-settings`.
//!
//! Design standards enforced (and verified by unit tests below):
//! - WCAG 2.2 SC 1.4.3 (>= 4.5:1 body text) and SC 1.4.11 (>= 3.0:1 interactive borders)
//! - Soft off-white (#E2E6EE) on off-black (#14171F) to prevent pure #FFF/#000 halation
//! - Desaturated accent (#7DB4FF) for the single primary CTA per view (Von Restorff Effect)
//! - Bounded section cards with generous inter-group spacing (Law of Common Region / Proximity)
//! - Taller primary hit targets separated from destructive buttons (Fitts's Law)

use eframe::egui;
use std::sync::atomic::{AtomicBool, Ordering};

pub const BG_BASE: [u8; 3] = [20, 23, 31]; // #14171F
pub const BG_SURFACE: [u8; 3] = [30, 34, 46]; // #1E222E
pub const BG_CONTROL: [u8; 3] = [40, 46, 61]; // #282E3D
pub const BG_HOVER: [u8; 3] = [52, 60, 79]; // #343C4F
pub const BORDER_UI: [u8; 3] = [100, 110, 130]; // #646E82 (>= 3:1 on BG_BASE)
pub const TEXT_PRIMARY: [u8; 3] = [226, 230, 238]; // #E2E6EE (avoids #FFF halation)
pub const TEXT_MUTED: [u8; 3] = [163, 172, 190]; // #A3ACBE (>= 4.5:1 on BG_SURFACE)
pub const ACCENT_PRIMARY: [u8; 3] = [125, 180, 255]; // #7DB4FF desaturated dark-theme accent
pub const ACCENT_TEXT: [u8; 3] = [15, 20, 30]; // #0F141E dark ink on accent CTA
pub const DANGER_BG: [u8; 3] = [150, 42, 48]; // #962A30
pub const DANGER_TEXT: [u8; 3] = [255, 235, 236]; // #FFEBEC

static THEME_APPLIED: AtomicBool = AtomicBool::new(false);

fn rgb(c: [u8; 3]) -> egui::Color32 {
    egui::Color32::from_rgb(c[0], c[1], c[2])
}

/// Apply the DIYPets WCAG-compliant dark theme once per GUI process.
pub fn apply_once(ctx: &egui::Context) {
    if THEME_APPLIED.swap(true, Ordering::Relaxed) {
        return;
    }
    let mut style = (*ctx.style_of(egui::Theme::Dark)).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 7.0);
    style.spacing.button_padding = egui::vec2(10.0, 5.0);
    style.spacing.indent = 16.0;
    style.spacing.interact_size.y = 26.0;

    let mut v = egui::Visuals::dark();
    v.panel_fill = rgb(BG_BASE);
    v.window_fill = rgb(BG_SURFACE);
    v.extreme_bg_color = rgb([14, 16, 22]);
    v.faint_bg_color = rgb(BG_SURFACE);
    v.override_text_color = Some(rgb(TEXT_PRIMARY));
    v.selection.bg_fill = egui::Color32::from_rgba_unmultiplied(125, 180, 255, 64);
    v.selection.stroke = egui::Stroke::new(1.5, rgb(ACCENT_PRIMARY));

    v.widgets.noninteractive.bg_fill = rgb(BG_SURFACE);
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, rgb(BORDER_UI));
    v.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, rgb(TEXT_PRIMARY));

    v.widgets.inactive.bg_fill = rgb(BG_CONTROL);
    v.widgets.inactive.weak_bg_fill = rgb(BG_CONTROL);
    v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, rgb(BORDER_UI));
    v.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, rgb(TEXT_PRIMARY));

    v.widgets.hovered.bg_fill = rgb(BG_HOVER);
    v.widgets.hovered.weak_bg_fill = rgb(BG_HOVER);
    v.widgets.hovered.bg_stroke = egui::Stroke::new(1.5, rgb(ACCENT_PRIMARY));
    v.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, rgb(TEXT_PRIMARY));

    v.widgets.active.bg_fill = rgb(ACCENT_PRIMARY);
    v.widgets.active.weak_bg_fill = rgb(ACCENT_PRIMARY);
    v.widgets.active.bg_stroke = egui::Stroke::new(1.5, rgb(TEXT_PRIMARY));
    v.widgets.active.fg_stroke = egui::Stroke::new(1.5, rgb(ACCENT_TEXT));

    style.visuals = v;
    ctx.set_style_of(egui::Theme::Dark, style.clone());
    ctx.set_style_of(egui::Theme::Light, style);
}

/// Von Restorff + Fitts's Law primary CTA button (full-width, 30px tall, desaturated accent fill).
pub fn primary_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    let text = egui::RichText::new(label).strong().color(rgb(ACCENT_TEXT));
    ui.add_sized(
        [ui.available_width().max(120.0), 30.0],
        egui::Button::new(text).fill(rgb(ACCENT_PRIMARY)),
    )
}

/// Compact destructive button styled clearly apart from primary actions (Fitts's Law / Error Prevention).
pub fn danger_small_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    let text = egui::RichText::new(label).strong().color(rgb(DANGER_TEXT));
    ui.add(egui::Button::new(text).fill(rgb(DANGER_BG)).small())
}

/// Law of Common Region: visually group related controls inside a bounded dark card.
pub fn section_card(ui: &mut egui::Ui, title: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::group(ui.style())
        .fill(rgb(BG_SURFACE))
        .stroke(egui::Stroke::new(1.0, rgb(BORDER_UI)))
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(title).strong().color(rgb(TEXT_PRIMARY)));
            ui.add_space(3.0);
            add_contents(ui);
        });
    ui.add_space(6.0);
}

pub fn muted_hint(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).size(11.5).color(rgb(TEXT_MUTED)));
}

/// Relative luminance per WCAG 2.2 sRGB specification.
pub fn relative_luminance(c: [u8; 3]) -> f64 {
    let lin = |ch: u8| {
        let s = ch as f64 / 255.0;
        if s <= 0.04045 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2])
}

/// Contrast ratio `(L1 + 0.05) / (L2 + 0.05)` per WCAG 2.2.
pub fn wcag_contrast_ratio(a: [u8; 3], b: [u8; 3]) -> f64 {
    let la = relative_luminance(a);
    let lb = relative_luminance(b);
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_theme_passes_wcag_aa_and_non_text_contrast() {
        assert!(
            wcag_contrast_ratio(TEXT_PRIMARY, BG_BASE) >= 12.0,
            "Primary text on base background should clear WCAG AAA"
        );
        assert!(
            wcag_contrast_ratio(TEXT_PRIMARY, BG_SURFACE) >= 10.0,
            "Primary text on card surface should clear WCAG AAA"
        );
        assert!(
            wcag_contrast_ratio(TEXT_MUTED, BG_SURFACE) >= 4.5,
            "Muted hint text must clear WCAG AA 4.5:1"
        );
        assert!(
            wcag_contrast_ratio(ACCENT_TEXT, ACCENT_PRIMARY) >= 7.0,
            "Primary CTA text must clear WCAG AAA 7:1"
        );
        assert!(
            wcag_contrast_ratio(DANGER_TEXT, DANGER_BG) >= 4.5,
            "Destructive button text must clear WCAG AA 4.5:1"
        );
        assert!(
            wcag_contrast_ratio(BORDER_UI, BG_BASE) >= 3.0,
            "UI borders must clear WCAG 2.2 SC 1.4.11 3.0:1 non-text contrast"
        );
    }
}
