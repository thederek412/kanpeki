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
    /// Custom editor background color (RGB, 0-255), if the user picked one.
    pub custom_bg: Option<[u8; 3]>,
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
            custom_bg: None,
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
        let bg = settings
            .custom_bg
            .map(|c| egui::Color32::from_rgb(c[0], c[1], c[2]))
            .unwrap_or_else(|| {
                if dark {
                    // Kanpeki's night ink.
                    egui::Color32::from_rgb(0x32, 0x36, 0x4a)
                } else {
                    // Kanpeki's day paper.
                    egui::Color32::from_rgb(0x93, 0x99, 0xb7)
                }
            });
        // Pick readable text for whatever background we ended up with.
        let [r, g, b, _] = bg.to_normalized_gamma_f32();
        let lum = 0.299 * r + 0.587 * g + 0.114 * b;
        let dark_bg = lum < 0.45;
        let text_fg = if dark_bg {
            egui::Color32::from_rgb(232, 232, 232)
        } else {
            egui::Color32::from_rgb(26, 26, 26)
        };
        // The current line is a shade of the background, never flat gray.
        let shift = |c: u8| -> u8 {
            if dark_bg {
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
                menu_fg: egui::Color32::from_rgb(220, 220, 220),
                text_bg: bg,
                text_fg,
                selection: egui::Color32::from_rgb(46, 92, 138),
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
                menu_fg: egui::Color32::from_rgb(30, 30, 30),
                text_bg: bg,
                text_fg,
                selection: egui::Color32::from_rgb(110, 150, 205),
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
        v.selection.bg_fill = self.selection;
        v.selection.stroke = egui::Stroke::new(1.0_f32, self.accent);
        v.hyperlink_color = self.accent;
        v
    }
}
