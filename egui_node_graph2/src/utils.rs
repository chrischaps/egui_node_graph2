pub trait ColorUtils {
    /// Multiplies the color rgb values by `factor`, keeping alpha untouched.
    fn lighten(&self, factor: f32) -> Self;
}

impl ColorUtils for egui::Color32 {
    fn lighten(&self, factor: f32) -> Self {
        egui::Color32::from_rgba_premultiplied(
            (self.r() as f32 * factor) as u8,
            (self.g() as f32 * factor) as u8,
            (self.b() as f32 * factor) as u8,
            self.a(),
        )
    }
}

/// Where the pointer is in `ui`'s layer. An app may draw the editor on a
/// layer it transforms (laid out at one zoom, shown at another), and the
/// pointer is read in screen points
pub(crate) fn layer_pointer(ui: &egui::Ui) -> Option<egui::Pos2> {
    let pos = ui.ctx().pointer_hover_pos()?;
    Some(match ui.ctx().layer_transform_from_global(ui.layer_id()) {
        Some(from_global) => from_global * pos,
        None => pos,
    })
}

/// Where `rect`, in `ui`'s layer, is on screen
pub(crate) fn on_screen(ui: &egui::Ui, rect: egui::Rect) -> egui::Rect {
    match ui.ctx().layer_transform_to_global(ui.layer_id()) {
        Some(to_global) => to_global * rect,
        None => rect,
    }
}

/// How many of `ui`'s layer's points make one screen point: a reach meant in
/// screen points, such as how near a cable the pointer picks it, times this
pub(crate) fn layer_points_per_point(ui: &egui::Ui) -> f32 {
    ui.ctx()
        .layer_transform_from_global(ui.layer_id())
        .map_or(1.0, |from_global| from_global.scaling)
}
