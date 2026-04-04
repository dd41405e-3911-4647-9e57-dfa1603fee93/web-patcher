use egui::Ui;

/// Returns true when the viewport is narrow enough to use mobile layout.
///
/// Uses `screen_rect` (the full canvas size in egui points) rather than
/// `content_rect` (which excludes already-placed panels and can be stale
/// on the first frame).  On high-DPI devices egui points map 1:1 to CSS
/// pixels, so 600 pt ≈ 600 CSS px regardless of `devicePixelRatio`.
pub(crate) fn is_mobile(ctx: &egui::Context) -> bool {
    ctx.viewport_rect().width() < 600.0
}

/// Returns the standard card-styled [`egui::Frame`] used throughout the UI.
pub(crate) fn card_frame(ui: &Ui) -> egui::Frame {
    let fill = if ui.visuals().dark_mode {
        ui.visuals().extreme_bg_color.gamma_multiply(1.3)
    } else {
        ui.visuals().extreme_bg_color
    };
    egui::Frame::new()
        .fill(fill)
        .corner_radius(6.0)
        .inner_margin(10.0)
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
}
