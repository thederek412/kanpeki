//! In-window dialogs: Open, Save, Find & Replace, background color, and About.
//! Drawn as modal panels over the editor, the way classic Mac dialogs sat
//! over their windows.

use std::path::{Path, PathBuf};

use egui_multiwin::egui;

use crate::fileio;
use crate::search::Search;
use crate::theme::Theme;

/// The open dialogs.
pub enum Dialog {
    /// File open dialog.
    Open(OpenDlg),
    /// Save / Save As dialog.
    Save(SaveDlg),
    /// Find & replace panel.
    Find {
        /// The search state.
        search: Search,
        /// An error message, if the query is bad.
        error: Option<String>,
    },
    /// Custom background color picker.
    Color {
        /// Red.
        r: u8,
        /// Green.
        g: u8,
        /// Blue.
        b: u8,
    },
    /// About box.
    About,
}

/// File open dialog state.
pub struct OpenDlg {
    /// The directory being listed.
    pub dir: PathBuf,
    /// The file name being typed.
    pub name: String,
    /// An error to display.
    pub error: Option<String>,
}

/// Save dialog state.
pub struct SaveDlg {
    /// The directory to save in.
    pub dir: PathBuf,
    /// The file name.
    pub name: String,
    /// An error to display.
    pub error: Option<String>,
}

/// What the user did in a dialog.
#[derive(Debug)]
pub enum Action {
    /// Nothing.
    None,
    /// Close the dialog.
    Close,
    /// Open this file.
    OpenFile(PathBuf),
    /// Save to this path.
    SaveFile(PathBuf),
    /// Find next/previous (forward?).
    FindNext(bool),
    /// Replace the current match.
    Replace,
    /// Replace every match.
    ReplaceAll,
    /// Apply a background color (r, g, b).
    ApplyColor(u8, u8, u8),
}

/// Draw a dialog and report the user's action.
pub fn show(ctx: &egui::Context, dlg: &mut Dialog, theme: &Theme) -> Action {
    let screen = ctx.screen_rect();
    match dlg {
        Dialog::Open(d) => dialog_box(ctx, "open_dialog", screen, theme, "Open", move |ui| {
            open_body(ui, d, theme)
        }),
        Dialog::Save(d) => dialog_box(ctx, "save_dialog", screen, theme, "Save", move |ui| {
            save_body(ui, d, theme)
        }),
        Dialog::Find { search, error } => dialog_box(ctx, "find_dialog", screen, theme, "Find & Replace", |ui| {
            find_body(ui, search, error, theme)
        }),
        Dialog::Color { r, g, b } => dialog_box(ctx, "color_dialog", screen, theme, "Background Color", |ui| {
            color_body(ui, r, g, b, theme)
        }),
        Dialog::About => dialog_box(ctx, "about_dialog", screen, theme, "About Kanpeki", |ui| {
            about_body(ui, theme);
            ui.add_space(10.0);
            let mut act = Action::None;
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), 26.0),
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    if ui.button("Close").clicked() {
                        act = Action::Close;
                    }
                },
            );
            act
        }),
    }
}

/// A centered, fixed-size dialog box. Returns the action from `body`.
fn dialog_box(
    ctx: &egui::Context,
    id: &'static str,
    screen: egui::Rect,
    theme: &Theme,
    title: &str,
    body: impl FnOnce(&mut egui::Ui) -> Action,
) -> Action {
    let size = egui::vec2(460.0, 340.0);
    let pos = screen.center() - size / 2.0;
    let mut action = Action::None;
    egui::Window::new(id)
        .id(egui::Id::new(id))
        .title_bar(false)
        .collapsible(false)
        .resizable(false)
        .movable(false)
        .fixed_pos(pos)
        .frame(
            egui::Frame::none()
                .fill(theme.dialog_bg)
                .stroke(egui::Stroke::new(1.0_f32, theme.border))
                .rounding(4.0)
                .inner_margin(egui::Margin::same(12.0)),
        )
        .show(ctx, |ui| {
            ui.set_width(size.x - 24.0);
            ui.strong(egui::RichText::new(title).size(15.0));
            ui.add_space(2.0);
            ui.separator();
            ui.add_space(4.0);
            action = body(ui);
        });
    action
}

fn open_body(ui: &mut egui::Ui, d: &mut OpenDlg, theme: &Theme) -> Action {
    ui.horizontal(|ui| {
        ui.label("Folder:");
        ui.label(egui::RichText::new(d.dir.display().to_string()).weak());
    });
    ui.horizontal(|ui| {
        if ui.button("Up one level").clicked() {
            if let Some(p) = d.dir.parent() {
                if p != Path::new("/") && !p.as_os_str().is_empty() {
                    d.dir = p.to_path_buf();
                }
            }
        }
    });
    ui.add_space(2.0);

    let (dirs, files) = fileio::dir_entries(&d.dir);
    egui::ScrollArea::vertical()
        .max_height(170.0)
        .show(ui, |ui| {
            for name in &dirs {
                if ui.button(format!("{name}/")).clicked() {
                    d.dir = d.dir.join(name);
                    d.name.clear();
                    return;
                }
            }
            for name in &files {
                if ui.button(name.as_str()).clicked() {
                    d.name = name.clone();
                }
            }
            if dirs.is_empty() && files.is_empty() {
                ui.label(egui::RichText::new("(empty folder)").weak());
            }
        });

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label("File:");
        ui.text_edit_singleline(&mut d.name);
    });
    if let Some(err) = &d.error {
        ui.label(egui::RichText::new(err).color(egui::Color32::from_rgb(200, 40, 40)));
    }
    ui.add_space(6.0);
    let act = ui
        .allocate_ui_with_layout(
            egui::vec2(ui.available_width(), 26.0),
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
            if ui.button("Cancel").clicked() {
                return Action::Close;
            }
            if ui.button("Open").clicked() {
                let p = if d.name.contains('/') || d.name.contains('\\') {
                    PathBuf::from(&d.name)
                } else {
                    d.dir.join(&d.name)
                };
                if p.is_dir() {
                    d.dir = p;
                    d.name.clear();
                    return Action::None;
                }
                if d.name.is_empty() {
                    d.error = Some("Type or choose a file name.".to_string());
                    return Action::None;
                }
                return Action::OpenFile(p);
            }
            Action::None
        })
        .inner;
    let _ = theme;
    act
}

fn save_body(ui: &mut egui::Ui, d: &mut SaveDlg, theme: &Theme) -> Action {
    ui.horizontal(|ui| {
        ui.label("Folder:");
        ui.label(egui::RichText::new(d.dir.display().to_string()).weak());
    });
    ui.horizontal(|ui| {
        if ui.button("Up one level").clicked() {
            if let Some(p) = d.dir.parent() {
                if p != Path::new("/") && !p.as_os_str().is_empty() {
                    d.dir = p.to_path_buf();
                }
            }
        }
    });
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label("File:");
        ui.text_edit_singleline(&mut d.name);
    });
    if let Some(err) = &d.error {
        ui.label(egui::RichText::new(err).color(egui::Color32::from_rgb(200, 40, 40)));
    }
    ui.add_space(8.0);
    let act = ui
        .allocate_ui_with_layout(
            egui::vec2(ui.available_width(), 26.0),
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
            if ui.button("Cancel").clicked() {
                return Action::Close;
            }
            if ui.button("Save").clicked() {
                if d.name.is_empty() {
                    d.error = Some("Type a file name.".to_string());
                    return Action::None;
                }
                let p = if d.name.contains('/') || d.name.contains('\\') {
                    PathBuf::from(&d.name)
                } else {
                    d.dir.join(&d.name)
                };
                return Action::SaveFile(p);
            }
            Action::None
        })
        .inner;
    let _ = theme;
    act
}

fn find_body(ui: &mut egui::Ui, s: &mut Search, error: &mut Option<String>, theme: &Theme) -> Action {
    ui.horizontal(|ui| {
        ui.label("Find:");
        ui.text_edit_singleline(&mut s.query);
    });
    ui.horizontal(|ui| {
        ui.label("Replace:");
        ui.text_edit_singleline(&mut s.replacement);
    });
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        ui.checkbox(&mut s.case_sensitive, "Match case");
        ui.checkbox(&mut s.whole_word, "Whole word");
        ui.checkbox(&mut s.regex, "Regex");
    });
    if let Some(err) = error.as_ref() {
        ui.label(egui::RichText::new(err).color(egui::Color32::from_rgb(200, 40, 40)));
    }
    ui.add_space(6.0);
    let act = ui
        .allocate_ui_with_layout(
            egui::vec2(ui.available_width(), 26.0),
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
            if ui.button("Cancel").clicked() {
                return Action::Close;
            }
            if ui.button("Replace All").clicked() {
                return Action::ReplaceAll;
            }
            if ui.button("Replace").clicked() {
                return Action::Replace;
            }
            if ui.button("< Previous").clicked() {
                return Action::FindNext(false);
            }
            if ui.button("Next >").clicked() {
                return Action::FindNext(true);
            }
            Action::None
        })
        .inner;
    let _ = theme;
    act
}

fn color_body(ui: &mut egui::Ui, r: &mut u8, g: &mut u8, b: &mut u8, theme: &Theme) -> Action {
    ui.label("Pick the background color of the editor:");
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let swatch = egui::vec2(28.0, 24.0);
        let (rect, _) = ui.allocate_exact_size(swatch, egui::Sense::hover());
        ui.painter().rect_filled(rect, 3.0, egui::Color32::from_rgb(*r, *g, *b));
        ui.painter().rect_stroke(
            rect,
            3.0,
            egui::Stroke::new(1.0_f32, theme.border),
        );
    });
    ui.add(egui::Slider::new(r, 0..=255).text("Red"));
    ui.add(egui::Slider::new(g, 0..=255).text("Green"));
    ui.add(egui::Slider::new(b, 0..=255).text("Blue"));
    ui.add_space(6.0);
    let act = ui
        .allocate_ui_with_layout(
            egui::vec2(ui.available_width(), 26.0),
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
            if ui.button("Cancel").clicked() {
                return Action::Close;
            }
            if ui.button("Apply").clicked() {
                return Action::ApplyColor(*r, *g, *b);
            }
            Action::None
        })
        .inner;
    let _ = theme;
    act
}

fn about_body(ui: &mut egui::Ui, _theme: &Theme) {
    let text = "Kanpeki
A fast, multi-window text editor for the desktop.

Named for the Japanese word for \"perfect\".
Created by Derek Kalinosky.
Inspired by Craig Marciniak's SaintEdit (1993),
the classic System 7 editor — thank you, Craig.

Deep undo, safe saves, find & replace with regex,
Day and Night colors, smooth scrolling, and support
for Unix, DOS and classic Mac line endings.

Built with Rust, egui and ropey.";
    for line in text.lines() {
        ui.label(egui::RichText::new(line).size(13.0));
    }
}
