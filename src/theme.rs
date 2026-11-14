//! Day/Night theming, in the spirit of SaintEdit's "Color Day/Night Settings".

use egui_multiwin::egui;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ThemeMode {
    Day,
    Night,
}

impl Default for ThemeMode {
    fn default() -> Self {
        ThemeMode::Day
    }
}

/// User-tweakable appearance settings, persisted to disk.
#[derive(Clone, Serialize, Deserialize)]
pub struct Settings {
    pub font_size: f32,
    pub theme_mode: ThemeMode,
    pub wrap: bool,
    /// Show line numbers in a left gutter.
    #[serde(default)]
    pub line_numbers: bool,
    pub recents: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            font_size: 15.0,
            theme_mode: ThemeMode::Day,
            wrap: false,
            line_numbers: false,
            recents: Vec::new(),
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        let path = Self::path();
        // Adopt the old app's settings once, if this name has none yet.
        if !path.exists() {
            if let Some(base) = dirs::config_dir() {
                let old = base.join("saintedit").join("settings.json");
                if old.exists() {
                    if let Some(parent) = path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    let _ = std::fs::copy(&old, &path);
                }
            }
        }
        match std::fs::read_to_string(&path) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(&path, json);
        }
    }

    fn path() -> std::path::PathBuf {
        let base = dirs::config_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
        base.join("kanpeki").join("settings.json")
    }

    pub fn add_recent(&mut self, path: &str) {
        if path.is_empty() {
            return;
        }
        self.recents.retain(|p| p != path);
        self.recents.insert(0, path.to_string());
        self.recents.truncate(8);
    }
}

/// A resolved color palette.
#[derive(Clone, Copy)]
pub struct Theme {
    pub mode: ThemeMode,
    pub chrome: egui::Color32,
    pub menu_bar: egui::Color32,
    pub menu_fg: egui::Color32,
    pub text_bg: egui::Color32,
    pub text_fg: egui::Color32,
    pub selection: egui::Color32,
    /// Text colour used on top of the selection.
    pub selection_fg: egui::Color32,
    /// Selection colour for egui widgets (text fields, menus).
    pub widget_selection: egui::Color32,
    /// Line-number colour.
    pub gutter_fg: egui::Color32,
    /// Hint text inside fields.
    pub hint_fg: egui::Color32,
    /// Small uppercase captions.
    pub caption_fg: egui::Color32,
    pub accent: egui::Color32,
    pub current_line: egui::Color32,
    pub status_bg: egui::Color32,
    pub dialog_bg: egui::Color32,
    pub border: egui::Color32,
    pub widget_fill: egui::Color32,
    pub widget_hover: egui::Color32,
}

impl Theme {
    pub fn resolve(settings: &Settings) -> Self {
        let dark = settings.theme_mode == ThemeMode::Night;
        // Day and Night are the only palettes. (A custom background option
        // used to sit on top of them; it made the two modes inconsistent and
        // has been removed.)
        let bg = if dark {
            // Kanpeki's night ink.
            egui::Color32::from_rgb(0x32, 0x36, 0x4a)
        } else {
            // Kanpeki's day paper.
            egui::Color32::from_rgb(0x93, 0x99, 0xb7)
        };
        // The two palettes are a duotone: Night is the Day paper on the night
        // ink, and Day is the night ink on the Day paper.
        let text_fg = if dark {
            egui::Color32::from_rgb(0x93, 0x99, 0xb7)
        } else {
            egui::Color32::from_rgb(0x32, 0x36, 0x4a)
        };
        // The current line is a shade of the background, never flat gray.
        let shift = |c: u8| -> u8 {
            if dark {
                c.saturating_add(14)
            } else {
                c.saturating_sub(14)
            }
        };
        let current_line =
            egui::Color32::from_rgb(shift(bg.r()), shift(bg.g()), shift(bg.b()));
        if dark {
            Self {
                mode: ThemeMode::Night,
                chrome: egui::Color32::from_rgb(30, 30, 30),
                menu_bar: egui::Color32::from_rgb(42, 42, 42),
                menu_fg: text_fg,
                text_bg: bg,
                text_fg,
                // Selecting swaps the duotone, like classic inverse video.
                selection: text_fg,
                selection_fg: bg,
                widget_selection: egui::Color32::from_rgb(30, 58, 95),
                gutter_fg: text_fg.gamma_multiply(0.80),
                hint_fg: text_fg.gamma_multiply(0.72),
                caption_fg: text_fg.gamma_multiply(0.75),
                accent: egui::Color32::from_rgb(111, 179, 255),
                current_line,
                status_bg: egui::Color32::from_rgb(38, 38, 38),
                dialog_bg: egui::Color32::from_rgb(44, 44, 44),
                border: egui::Color32::from_rgb(66, 66, 66),
                widget_fill: egui::Color32::from_rgb(54, 54, 54),
                widget_hover: egui::Color32::from_rgb(74, 74, 74),
            }
        } else {
            Self {
                mode: ThemeMode::Day,
                chrome: egui::Color32::from_rgb(180, 180, 180),
                menu_bar: egui::Color32::from_rgb(200, 200, 200),
                menu_fg: text_fg,
                text_bg: bg,
                text_fg,
                selection: egui::Color32::from_rgb(110, 150, 205),
                selection_fg: text_fg,
                widget_selection: egui::Color32::from_rgb(110, 150, 205),
                gutter_fg: text_fg.gamma_multiply(0.55),
                hint_fg: text_fg.gamma_multiply(0.45),
                caption_fg: text_fg.gamma_multiply(0.62),
                accent: egui::Color32::from_rgb(0, 96, 160),
                current_line,
                status_bg: egui::Color32::from_rgb(192, 192, 192),
                dialog_bg: egui::Color32::from_rgb(216, 216, 216),
                border: egui::Color32::from_rgb(128, 128, 128),
                widget_fill: egui::Color32::from_rgb(228, 228, 228),
                widget_hover: egui::Color32::from_rgb(246, 246, 246),
            }
        }
    }

    pub fn egui_visuals(&self) -> egui::Visuals {
        let dark = self.mode == ThemeMode::Night;
        let mut v = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };
        v.dark_mode = dark;
        v.window_fill = self.dialog_bg;
        v.panel_fill = self.menu_bar;
        v.extreme_bg_color = self.menu_bar;
        v.widgets.noninteractive.bg_fill = self.menu_bar;
        v.widgets.noninteractive.bg_stroke = egui::Stroke::NONE;
        v.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0_f32, self.menu_fg);
        v.widgets.inactive.bg_fill = self.widget_fill;
        v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, self.border);
        v.widgets.inactive.fg_stroke = egui::Stroke::new(1.0_f32, self.menu_fg);
        v.widgets.inactive.weak_bg_fill = self.widget_fill;
        v.widgets.hovered.bg_fill = self.widget_hover;
        v.widgets.hovered.weak_bg_fill = self.widget_hover;
        v.widgets.hovered.fg_stroke = egui::Stroke::new(1.0_f32, self.menu_fg);
        v.widgets.active.bg_fill = self.widget_hover;
        v.widgets.active.fg_stroke = egui::Stroke::new(1.0_f32, self.menu_fg);
        v.selection.bg_fill = self.widget_selection;
        v.selection.stroke = egui::Stroke::new(1.0_f32, self.accent);
        v.hyperlink_color = self.accent;
        v
    }
}
