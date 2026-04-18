mod presets;

use egui::Color32;

pub use presets::PRESETS;

/// Theme definition for the markdown reader
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct Theme {
    pub name: &'static str,
    pub is_dark: bool,
    // Base
    pub background: Color32,
    pub foreground: Color32,
    // Semantic
    pub heading: Color32,
    pub link: Color32,
    pub link_hover: Color32,
    pub code_bg: Color32,
    pub code_fg: Color32,
    pub quote_border: Color32,
    pub quote_fg: Color32,
    pub table_border: Color32,
    pub table_header_bg: Color32,
    pub table_stripe_bg: Option<Color32>,
    pub hr_color: Color32,
    pub list_marker: Color32,
    pub task_checked: Color32,
    pub task_unchecked: Color32,
    pub selection_bg: Color32,
    // Code syntax (syntect theme name)
    pub syntax_theme: &'static str,
}

impl Theme {
    /// Get the default theme
    pub fn default_theme() -> &'static Theme {
        &PRESETS[0]
    }

    /// Get all preset themes
    pub fn all_themes() -> &'static [Theme] {
        &*PRESETS
    }

    /// Muted text color (for placeholder text)
    pub fn muted_text(&self) -> Color32 {
        let fg = self.foreground;
        Color32::from_rgba_unmultiplied(fg.r(), fg.g(), fg.b(), 80)
    }

    /// Get heading size for a given level
    pub fn heading_size(&self, level: u8, base: f32) -> f32 {
        match level {
            1 => base * 2.0,
            2 => base * 1.5,
            3 => base * 1.25,
            4 => base * 1.1,
            5 => base * 1.0,
            _ => base * 0.9,
        }
    }
}
