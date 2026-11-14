//! In-window dialogs: Open, Save, Find & Replace, background color, and About.
//! Drawn as modal panels over the editor, the way classic Mac dialogs sat
//! over their windows.

use std::path::{Path, PathBuf};

use egui_multiwin::egui;

use crate::fileio;
use crate::search::Search;
use crate::theme::{Theme, ThemeMode};

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
        /// The search field has not been focused yet.
        focus: bool,
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
    /// A path that is about to be overwritten; a second Save confirms it.
    pub confirm: Option<PathBuf>,
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
}

/// Draw a dialog and report the user's action.
pub fn show(ctx: &egui::Context, dlg: &mut Dialog, theme: &Theme) -> Action {
    match dlg {
        Dialog::Open(d) => dialog_box(ctx, "open_dialog", theme, "Open", move |ui| {
            open_body(ui, d, theme)
        }),
        Dialog::Save(d) => dialog_box(ctx, "save_dialog", theme, "Save", move |ui| {
            save_body(ui, d, theme)
        }),
        Dialog::Find { search, error, focus } => dialog_box(ctx, "find_dialog", theme, "Find & Replace", |ui| {
            find_body(ui, search, error, theme, focus)
        }),
        Dialog::About => dialog_box(ctx, "about_dialog", theme, "About Kanpeki", |ui| {
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

/// A centered dialog box with a title rule and a soft drop shadow. Returns the
/// action from `body`.
fn dialog_box(
    ctx: &egui::Context,
    id: &'static str,
    theme: &Theme,
    title: &str,
    body: impl FnOnce(&mut egui::Ui) -> Action,
) -> Action {
    const WIDTH: f32 = 480.0;
    const MARGIN: f32 = 16.0;

    let mut action = Action::None;
    egui::Window::new(id)
        .id(egui::Id::new(id))
        .title_bar(false)
        .collapsible(false)
        .resizable(false)
        .movable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .frame(
            egui::Frame::none()
                .fill(theme.dialog_bg)
                .stroke(egui::Stroke::new(1.0_f32, theme.border))
                .rounding(6.0)
                .inner_margin(egui::Margin::same(MARGIN))
                .shadow(egui::epaint::Shadow {
                    offset: egui::vec2(0.0, 6.0),
                    blur: 18.0,
                    spread: 0.0,
                    color: egui::Color32::from_black_alpha(70),
                }),
        )
        .show(ctx, |ui| {
            ui.set_width(WIDTH - 2.0 * MARGIN);
            ui.spacing_mut().item_spacing = egui::vec2(8.0, 7.0);

            // A small accent rule, then the title.
            ui.horizontal(|ui| {
                let (bar, _) =
                    ui.allocate_exact_size(egui::vec2(3.0, 17.0), egui::Sense::hover());
                ui.painter().rect_filled(bar, 1.5, theme.accent);
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(title)
                        .size(15.0)
                        .strong()
                        .color(theme.text_fg),
                );
            });

            // A hairline under the title (the egui separator is disabled by
            // the theme's noninteractive stroke settings).
            ui.add_space(5.0);
            let y = ui.cursor().min.y;
            painter_rule(
                ui,
                egui::Rect::from_min_size(
                    egui::pos2(ui.max_rect().left(), y),
                    egui::vec2(ui.available_width(), 1.0),
                ),
                theme.border.gamma_multiply(0.7),
            );
            ui.add_space(10.0);

            action = body(ui);
        });
    action
}

/// Paint a one-pixel rule.
fn painter_rule(ui: &egui::Ui, rect: egui::Rect, color: egui::Color32) {
    ui.painter().rect_filled(rect, 0.0, color);
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
                if p.exists() && d.confirm.as_ref() != Some(&p) {
                    // Saving over an existing file takes a second click.
                    d.confirm = Some(p.clone());
                    d.error = Some(format!(
                        "{} already exists. Press Save again to replace it.",
                        p.display()
                    ));
                    return Action::None;
                }
                return Action::SaveFile(p);
            }
            Action::None
        })
        .inner;
    let _ = theme;
    act
}

fn find_body(
    ui: &mut egui::Ui,
    s: &mut Search,
    error: &mut Option<String>,
    theme: &Theme,
    focus: &mut bool,
) -> Action {
    // Text fields sit on the dialog's control color rather than egui's
    // default "extreme" background.
    ui.visuals_mut().extreme_bg_color = theme.widget_fill;
    ui.spacing_mut().button_padding = egui::vec2(10.0, 5.0);

    let field = |ui: &mut egui::Ui, text: &mut String, hint: &str| {
        ui.add(
            egui::TextEdit::singleline(text)
                .hint_text(
                    egui::RichText::new(hint).size(13.0).color(theme.hint_fg),
                )
                .margin(egui::Margin::symmetric(8.0, 6.0))
                .desired_width(f32::INFINITY),
        )
    };
    let caption = |ui: &mut egui::Ui, text: &str| {
        ui.label(
            egui::RichText::new(text)
                .size(10.0)
                .strong()
                .color(theme.caption_fg),
        );
    };

    caption(ui, "FIND");
    let find_resp = field(ui, &mut s.query, "Text or regular expression");
    if *focus {
        // Focus the query and select it, so reopening the dialog and typing
        // replaces the previous term (as in any file manager or editor).
        find_resp.request_focus();
        if let Some(mut state) =
            egui::widgets::text_edit::TextEditState::load(ui.ctx(), find_resp.id)
        {
            let n = s.query.chars().count();
            state.cursor.set_char_range(Some(egui::text::CCursorRange::two(
                egui::text::CCursor::new(0),
                egui::text::CCursor::new(n),
            )));
            state.store(ui.ctx(), find_resp.id);
        }
        *focus = false;
    }
    if find_resp.changed() {
        // A fresh term makes the old "no matches" verdict stale.
        *error = None;
    }

    ui.add_space(3.0);
    caption(ui, "REPLACE WITH");
    field(ui, &mut s.replacement, "Replacement text (optional)");

    ui.add_space(5.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 18.0;
        ui.checkbox(&mut s.case_sensitive, "Match case");
        ui.checkbox(&mut s.whole_word, "Whole word");
        ui.checkbox(&mut s.regex, "Regex");
    });

    // Status line: reserved even when empty, so the buttons never jump.
    ui.add_space(2.0);
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), 17.0),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            if let Some(msg) = error.as_ref() {
                // Errors are red, successful replacements green, plain
                // information (such as "No matches found.") muted.
                let color = if msg.starts_with("Invalid") {
                    match theme.mode {
                        ThemeMode::Night => egui::Color32::from_rgb(255, 140, 140),
                        ThemeMode::Day => egui::Color32::from_rgb(168, 38, 38),
                    }
                } else if msg.starts_with("Replaced") {
                    match theme.mode {
                        ThemeMode::Night => egui::Color32::from_rgb(124, 204, 143),
                        ThemeMode::Day => egui::Color32::from_rgb(28, 110, 52),
                    }
                } else {
                    theme.caption_fg
                };
                ui.label(egui::RichText::new(msg).size(12.0).color(color));
            }
        },
    );
    ui.add_space(2.0);

    let has_query = !s.query.trim().is_empty();
    let mut act = Action::None;
    ui.horizontal(|ui| {
        if ui
            .add_enabled(has_query, egui::Button::new("Previous"))
            .clicked()
        {
            act = Action::FindNext(false);
        }
        let next = egui::Button::new(
            egui::RichText::new("Next").color(on_accent(theme)),
        )
        .fill(theme.accent);
        if ui.add_enabled(has_query, next).clicked() {
            act = Action::FindNext(true);
        }
        ui.with_layout(
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
                if ui.button("Close").clicked() {
                    act = Action::Close;
                }
                if ui
                    .add_enabled(has_query, egui::Button::new("Replace All"))
                    .clicked()
                {
                    act = Action::ReplaceAll;
                }
                if ui
                    .add_enabled(has_query, egui::Button::new("Replace"))
                    .clicked()
                {
                    act = Action::Replace;
                }
            },
        );
    });

    // Enter finds the next match; Shift+Enter goes back.
    let (enter, shift) = ui.input(|i| (i.key_pressed(egui::Key::Enter), i.modifiers.shift));
    if matches!(act, Action::None) && enter && has_query {
        act = Action::FindNext(!shift);
    }
    // Clicking a button (or pressing Enter) must not leave the dialog without a
    // keyboard target: put the caret back in the query field.
    if !matches!(act, Action::None | Action::Close) {
        find_resp.request_focus();
    }
    act
}

/// Readable text on the accent-colored primary button.
fn on_accent(theme: &Theme) -> egui::Color32 {
    match theme.mode {
        ThemeMode::Night => egui::Color32::from_rgb(18, 22, 30),
        ThemeMode::Day => egui::Color32::WHITE,
    }
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

Built with Rust, egui and ropey.
Set in Hurmit Nerd Font Mono (SIL OFL 1.1).";
    for line in text.lines() {
        ui.label(egui::RichText::new(line).size(13.0));
    }
}
