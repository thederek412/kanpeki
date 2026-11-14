//! One document window: menu bar, custom-drawn editor, status bar and
//! dialogs — one true OS window per document.

use std::path::PathBuf;
use std::sync::mpsc;

use egui_multiwin::arboard;
use egui_multiwin::egui;
use egui_multiwin::egui::{Align2, FontFamily, FontId, Key, Pos2, Rect};
use egui_multiwin::egui_glow::EguiGlow;
use egui_multiwin::winit::window::{Window, WindowId};

use crate::buffer::Doc;
use crate::dialogs::{self, Action, Dialog};
use crate::editor::{self, Scroll, Selection};
use crate::fileio;
use crate::{AppCommon, CustomEvent, Windows};
use crate::search::Search;
use crate::theme::{Theme, ThemeMode};

use crate::egui_multiwin_dynamic::multi_window::NewWindowRequest;
use crate::egui_multiwin_dynamic::tracked_window::{RedrawResponse, TrackedWindow};

/// What the user chose in the "unsaved changes" prompt.
enum CloseAct {
    /// Keep the window open.
    Cancel,
    /// Close and discard the changes.
    Discard,
    /// Save, then close.
    Save,
}

/// Cache key for the visual-line table.
#[derive(PartialEq)]
struct MapKey {
    /// `Doc::edits` generation.
    edits: u64,
    /// Word wrap on?
    wrap: bool,
    /// Column count used.
    cols: usize,
    /// Font size (x2 to stay integral).
    font: u32,
}

impl MapKey {
    /// A key that never matches.
    const INVALID: MapKey = MapKey {
        edits: u64::MAX,
        wrap: false,
        cols: 0,
        font: 0,
    };
}

/// The state of one document window.
pub struct DocWindow {
    /// The document being edited.
    doc: Doc,
    /// Caret + selection, in character indices.
    sel: Selection,
    /// Inertial vertical scroll.
    scroll: Scroll,
    /// Horizontal offset, in pixels.
    scroll_x: f32,
    /// Visual-line table.
    map: editor::VisualMap,
    /// Cache key of `map`.
    map_key: MapKey,
    /// Open dialog, if any.
    dialog: Option<Dialog>,
    /// Remembered find/replace state.
    find: Search,
    /// The current match (for "Replace").
    find_target: Option<(usize, usize)>,
    /// The unsaved-changes prompt is on screen.
    close_prompt: bool,
    /// Close even if the document is dirty (the user decided).
    force_close: bool,
    /// A Save-As chosen from the close prompt should close the window after saving.
    save_then_close: bool,
    /// A system file picker is running in the background; receives the chosen path.
    pending_open: Option<mpsc::Receiver<Option<PathBuf>>>,
    /// A system save dialog is running in the background; receives the chosen path.
    pending_save: Option<mpsc::Receiver<Option<PathBuf>>>,
    /// Winit id of this window (set on first redraw).
    wid: Option<WindowId>,
    /// The last title we set on the window.
    last_title: String,
    /// Word count (from the worker thread) and the generation it belongs to.
    words: Option<usize>,
    words_gen: u64,
    /// Bumped whenever a different document is loaded into this window.
    load_seq: u64,
    words_tx: mpsc::Sender<(u64, usize)>,
    words_rx: mpsc::Receiver<(u64, usize)>,
    /// A selection drag is in progress.
    dragging: bool,
    /// The scrollbar thumb is being dragged.
    sb_dragging: bool,
    /// Pointer offset within the thumb when the drag started.
    sb_grab: f32,
    /// The pointer is over the scrollbar.
    sb_hover: bool,
    /// One-shot startup focus repair has run.
    focus_nudged: bool,
    /// A character to bring into view on the next draw (for edits that run
    /// before the editor's geometry is known, e.g. from the menus).
    pending_scroll: Option<usize>,
    /// A transient note for the status bar.
    note: Option<String>,
}

/// The display name of a document.
fn doc_title(doc: &Doc) -> String {
    doc.path
        .as_ref()
        .and_then(|p| p.file_name())
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Untitled".to_string())
}

/// The file name (only) of a path string.
fn short_name(p: &str) -> String {
    std::path::Path::new(p)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.to_string())
}

impl DocWindow {
    /// A fresh window for `doc`.
    pub fn new(doc: Doc) -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            doc,
            sel: Selection::default(),
            scroll: Scroll::new(),
            scroll_x: 0.0,
            map: editor::VisualMap { lines: Vec::new() },
            map_key: MapKey::INVALID,
            dialog: None,
            find: Search::default(),
            find_target: None,
            close_prompt: false,
            force_close: false,
            save_then_close: false,
            pending_open: None,
            pending_save: None,
            wid: None,
            last_title: String::new(),
            words: None,
            words_gen: u64::MAX,
            load_seq: 0,
            words_tx: tx,
            words_rx: rx,
            dragging: false,
            sb_dragging: false,
            sb_grab: 0.0,
            sb_hover: false,
            focus_nudged: false,
            pending_scroll: None,
            note: None,
        }
    }

    /// Build a `NewWindowRequest` for a new document window (cascaded position).
    pub fn request(doc: Doc, c: &mut AppCommon) -> NewWindowRequest {
        let seq = c.window_seq;
        c.window_seq += 1;
        let title = doc_title(&doc);
        let builder = egui_multiwin::winit::window::WindowBuilder::new()
            .with_title(format!("{title} - Kanpeki"))
            .with_inner_size(egui_multiwin::winit::dpi::PhysicalSize::new(1024, 720))
            .with_position(egui_multiwin::winit::dpi::PhysicalPosition::new(
                70 + (seq % 10) * 36,
                50 + (seq % 10) * 28,
            ))
            .with_window_icon(crate::icon::app_icon());
        NewWindowRequest::new(
            Windows::Doc(Self::new(doc)),
            builder,
            egui_multiwin::tracked_window::TrackedWindowOptions { vsync: true, shader: None },
            egui_multiwin::multi_window::new_id(),
        )
    }

    /// The title-bar text for this document.
    fn title_string(&self) -> String {
        let name = doc_title(&self.doc);
        if self.doc.modified {
            format!("* {name}")
        } else {
            name
        }
    }

    /// Ask to close this window (prompting first if the document is dirty).
    fn begin_close(&mut self, rr: &mut RedrawResponse) {
        if self.doc.modified {
            self.close_prompt = true;
        } else {
            rr.quit = true;
        }
    }

    /// Save the document to its current path (or ask for one).
    fn save_current(&mut self, c: &mut AppCommon, rr: &mut RedrawResponse) {
        if let Some(path) = self.doc.path.clone() {
            self.save_to(path, c, rr);
        } else {
            self.begin_save();
        }
    }

    /// Write the document to `path` and update the window/settings.
    fn save_to(&mut self, path: PathBuf, c: &mut AppCommon, rr: &mut RedrawResponse) {
        match fileio::save(&self.doc, &path) {
            Ok(()) => {
                self.doc.path = Some(path.clone());
                self.doc.modified = false;
                if let Some(s) = path.to_str() {
                    c.settings.add_recent(s);
                    c.settings.save();
                }
                self.note = Some(format!("Saved {}", path.display()));
                if self.save_then_close {
                    self.save_then_close = false;
                    self.force_close = true;
                    rr.quit = true;
                }
            }
            Err(e) => self.note = Some(e),
        }
    }

    /// Ask for a file name: the system dialog if available, else the built-in one.
    fn begin_save(&mut self) {
        if self.pending_save.is_some() || self.pending_open.is_some() {
            return;
        }
        let dir = self
            .doc
            .path
            .as_ref()
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf())
            .unwrap_or_else(fileio::default_dir);
        let name = self
            .doc
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();

        if !zenity_available() {
            self.dialog = Some(Dialog::Save(dialogs::SaveDlg { dir, name, error: None }));
            return;
        }

        let suggested = if name.is_empty() {
            dir.join("Untitled")
        } else {
            dir.join(&name)
        };
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let out = std::process::Command::new("zenity")
                .arg("--file-selection")
                .arg("--save")
                .arg("--title=Save File")
                .arg(format!("--filename={}", suggested.display()))
                .output();
            let path = match out {
                Ok(o) if o.status.success() => {
                    let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                    if s.is_empty() {
                        None
                    } else {
                        Some(PathBuf::from(s))
                    }
                }
                _ => None,
            };
            let _ = tx.send(path);
        });
        self.pending_save = Some(rx);
        self.note = Some("Choose where to save…".to_string());
    }

    /// Open (or load in place) a file from disk.
    fn open_path(&mut self, path: std::path::PathBuf, c: &mut AppCommon, rr: &mut RedrawResponse) {
        self.dialog = None;
        match fileio::load(&path) {
            Ok(l) => {
                let doc = Doc::from_text(&l.text, l.eol, l.encoding, Some(path.clone()));
                if self.doc.len_chars() == 0 && !self.doc.modified {
                    // This window is a fresh untitled one: load in place.
                    self.doc = doc;
                    // A new document starts at edits 0, which could alias the
                    // cached visual map of the empty document. Force a rebuild.
                    self.doc.edits = self.doc.edits.wrapping_add(1);
                    self.load_seq = self.load_seq.wrapping_add(1);
                    self.words = None;
                    self.sel.collapse(0);
                    self.scroll.offset = 0.0;
                    self.scroll.velocity = 0.0;
                    self.scroll_x = 0.0;
                    self.find_target = None;
                } else {
                    rr.new_windows.push(DocWindow::request(doc, c));
                }
                if let Some(s) = path.to_str() {
                    c.settings.add_recent(s);
                    c.settings.save();
                }
            }
            Err(e) => self.note = Some(e),
        }
    }

    /// Open the find & replace dialog.
    fn open_find(&mut self) {
        self.dialog = Some(Dialog::Find {
            search: self.find.clone(),
            error: None,
            focus: true,
        });
    }

    /// Open a file: the system picker if available, else the built-in dialog.
    fn begin_open(&mut self) {
        if self.pending_open.is_some() {
            return;
        }
        let start = self
            .doc
            .path
            .as_ref()
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf())
            .unwrap_or_else(fileio::default_dir);

        if !zenity_available() {
            self.dialog = Some(Dialog::Open(dialogs::OpenDlg {
                dir: start,
                name: String::new(),
                error: None,
            }));
            return;
        }

        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let dir = format!("{}/", start.display());
            let out = std::process::Command::new("zenity")
                .arg("--file-selection")
                .arg("--title=Open File")
                .arg(format!("--filename={dir}"))
                .output();
            let path = match out {
                Ok(o) if o.status.success() => {
                    let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                    if s.is_empty() {
                        None
                    } else {
                        Some(PathBuf::from(s))
                    }
                }
                _ => None,
            };
            let _ = tx.send(path);
        });
        self.pending_open = Some(rx);
        self.note = Some("Choose a file…".to_string());
    }

    /// Collect the system save dialog's result when it finishes.
    fn poll_save_picker(&mut self, c: &mut AppCommon, rr: &mut RedrawResponse, window: &Window) {
        if self.pending_save.is_none() {
            return;
        }
        let msg = self.pending_save.as_ref().map(|rx| rx.try_recv());
        match msg {
            Some(Ok(Some(path))) => {
                self.pending_save = None;
                self.note = None;
                self.save_to(path, c, rr);
            }
            Some(Ok(None)) => {
                self.pending_save = None;
                // Cancelling the save from the close prompt must not close.
                self.save_then_close = false;
                self.note = Some("Save cancelled.".to_string());
            }
            Some(Err(mpsc::TryRecvError::Empty)) => {
                window.request_redraw();
            }
            _ => {
                self.pending_save = None;
            }
        }
    }

    /// Collect the system picker's result when it finishes.
    fn poll_open_picker(&mut self, c: &mut AppCommon, rr: &mut RedrawResponse, window: &Window) {
        if self.pending_open.is_none() {
            return;
        }
        let msg = self.pending_open.as_ref().map(|rx| rx.try_recv());
        match msg {
            Some(Ok(Some(path))) => {
                self.pending_open = None;
                self.note = None;
                self.open_path(path, c, rr);
            }
            Some(Ok(None)) => {
                self.pending_open = None;
                self.note = Some("Open cancelled.".to_string());
            }
            Some(Err(mpsc::TryRecvError::Empty)) => {
                // Keep polling so the file appears the moment the picker closes.
                window.request_redraw();
            }
            _ => {
                self.pending_open = None;
            }
        }
    }
}

/// Is a system file dialog available (zenity on XFCE/GNOME)?
fn zenity_available() -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|d| d.join("zenity").is_file()))
        .unwrap_or(false)
}

impl TrackedWindow for DocWindow {
    fn is_root(&self) -> bool {
        // Every document window is a root: closing the last one ends the app.
        true
    }

    fn can_quit(&mut self, c: &mut AppCommon) -> bool {
        // Called twice on a close attempt (try_quit, then do_window_events),
        // so keep this idempotent.
        if self.doc.modified && !self.force_close {
            // Don't close yet: the prompt is drawn on the next redraw.
            self.close_prompt = true;
            false
        } else {
            if let Some(id) = self.wid {
                c.remove_entry(id);
            }
            true
        }
    }

    fn custom_event(
        &mut self,
        _event: &CustomEvent,
        _c: &mut AppCommon,
        _egui: &mut EguiGlow,
        _window: &Window,
        _clipboard: &mut arboard::Clipboard,
    ) -> RedrawResponse {
        // This app keeps all window communication local to the window, so
        // routed custom events only need to be acknowledged.
        RedrawResponse::default()
    }

    fn redraw(
        &mut self,
        c: &mut AppCommon,
        egui: &mut EguiGlow,
        window: &Window,
        clipboard: &mut arboard::Clipboard,
    ) -> RedrawResponse {
        let ctx = egui.egui_ctx.clone();
        let theme = Theme::resolve(&c.settings);
        ctx.set_visuals(theme.egui_visuals());

        // --- Bookkeeping: Window menu entry, title bar, active window.
        let wid = window.id();
        self.wid = Some(wid);
        let title = self.title_string();
        c.upsert_entry(wid, &title, self.doc.modified);
        if window.has_focus() {
            c.active_window = Some(wid);
        }
        let wm_title = format!("{title} - Kanpeki");
        if wm_title != self.last_title {
            window.set_title(&wm_title);
            self.last_title = wm_title;
        }

        // A window the WM focuses while it is being mapped can miss winit's
        // `FocusIn`, which leaves winit's active window unset and silently drops
        // every key event (typing and shortcuts alike). A single hide/show cycle
        // makes the WM deliver `FocusIn` again; do it once, on the first frame.
        if !self.focus_nudged {
            self.focus_nudged = true;
            if !window.has_focus() {
                window.set_visible(false);
                window.set_visible(true);
                window.focus_window();
                window.request_redraw();
            }
        }

        let mut rr = RedrawResponse::default();
        let modal = self.close_prompt || self.dialog.is_some();

        // Background system dialogs may have returned a path.
        self.poll_open_picker(c, &mut rr, window);
        self.poll_save_picker(c, &mut rr, window);

        // While a modal is up, only Escape (and the dialog's own widgets) act.
        if modal {
            self.handle_modal_keys(&ctx);
        }

        // --- Menu bar.
        egui::TopBottomPanel::top("menubar")
            .frame(egui::Frame::none().fill(theme.menu_bar))
            .show(&ctx, |ui| {
                self.menu_bar(ui, c, &theme, modal, clipboard, &mut rr);
            });

        // --- Editor.
        let font_id = FontId::new(c.settings.font_size, FontFamily::Monospace);
        let (row_h, glyph_w) = ctx.fonts(|f| (f.row_height(&font_id), f.glyph_width(&font_id, 'x')));
        let wrap = c.settings.wrap;
        let font_size = c.settings.font_size;

        let mut editor_rect = Rect::NOTHING;
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(theme.text_bg))
            .show(&ctx, |ui| {
                let rect = ui.max_rect();
                editor_rect = rect;
                self.draw_editor(ui, &theme, &font_id, row_h, glyph_w, rect, wrap, font_size, c.settings.line_numbers);
                if !modal {
                    self.handle_editor_input(c, &ctx, rect, row_h, glyph_w, clipboard, &mut rr);
                }
            });

        // --- Status bar.
        egui::TopBottomPanel::bottom("statusbar")
            .frame(egui::Frame::none().fill(theme.status_bg))
            .show(&ctx, |ui| {
                self.status_bar(ui, c, &theme);
            });

        // --- Dialogs (foreground layers, drawn after the panels).
        if self.close_prompt {
            if let Some(act) = self.close_dialog(&ctx, &theme) {
                self.do_close_act(act, c, &mut rr);
            }
        } else {
            let act = self.dialog.as_mut().map(|d| dialogs::show(&ctx, d, &theme));
            if let Some(act) = act {
                self.apply_action(act, c, &mut rr);
            }
        }

        // --- Smooth scrolling: keep the event loop awake while gliding.
        let dt = ctx.input(|i| i.stable_dt);
        if self.step_scroll(editor_rect.height(), row_h, dt) {
            window.request_redraw();
        }

        rr
    }
}

impl DocWindow {
    /// The classic menu bar across the top of the window.
    fn menu_bar(
        &mut self,
        ui: &mut egui::Ui,
        c: &mut AppCommon,
        theme: &Theme,
        modal: bool,
        clipboard: &mut arboard::Clipboard,
        rr: &mut RedrawResponse,
    ) {
        ui.spacing_mut().button_padding = egui::vec2(8.0, 4.0);
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            if modal {
                // A modal dialog owns the interaction; the menus are inert.
                for name in ["File", "Edit", "View", "Window", "Help"] {
                    ui.add_enabled(
                        false,
                        egui::Label::new(egui::RichText::new(name).color(theme.menu_fg)),
                    );
                    ui.add_space(4.0);
                }
            } else {
                ui.menu_button("File", |ui| self.file_menu(ui, c, rr));
                ui.menu_button("Edit", |ui| self.edit_menu(ui, clipboard));
                ui.menu_button("View", |ui| self.view_menu(ui, c));
                ui.menu_button("Window", |ui| self.window_menu(ui, c));
                ui.menu_button("Help", |ui| self.help_menu(ui));
            }
            ui.add_space(6.0);
        });
    }

    /// File: new/open/save/close document windows.
    fn file_menu(&mut self, ui: &mut egui::Ui, c: &mut AppCommon, rr: &mut RedrawResponse) {
        if ui.button("New (Ctrl+N)").clicked() {
            rr.new_windows.push(DocWindow::request(Doc::blank(), c));
            ui.close_menu();
        }
        if ui.button("Open… (Ctrl+O)").clicked() {
            self.begin_open();
            ui.close_menu();
        }

        let recents = c.settings.recents.clone();
        ui.menu_button("Open Recent", |ui| {
            if recents.is_empty() {
                ui.add_enabled(false, egui::Label::new("(no recent files)"));
            } else {
                for r in &recents {
                    if ui
                        .button(short_name(r))
                        .on_hover_text(r.as_str())
                        .clicked()
                    {
                        self.open_path(std::path::PathBuf::from(r.as_str()), c, rr);
                        ui.close_menu();
                    }
                }
            }
        });

        ui.separator();
        if ui.button("Close (Ctrl+W)").clicked() {
            self.begin_close(rr);
            ui.close_menu();
        }
        if ui.button("Save (Ctrl+S)").clicked() {
            self.save_current(c, rr);
            ui.close_menu();
        }
        if ui.button("Save As…").clicked() {
            self.begin_save();
            ui.close_menu();
        }
        ui.separator();
        if ui.button("Quit Kanpeki").clicked() {
            // Close through the unsaved-changes prompt; the app exits when
            // the last window is gone.
            self.begin_close(rr);
            ui.close_menu();
        }
    }

    /// Edit: undo/redo, clipboard, selection, and find & replace.
    fn edit_menu(&mut self, ui: &mut egui::Ui, clipboard: &mut arboard::Clipboard) {
        let can_undo = self.doc.can_undo();
        if ui
            .add_enabled(can_undo, egui::Button::new("Undo (Ctrl+Z)"))
            .clicked()
        {
            self.do_undo();
            ui.close_menu();
        }
        let can_redo = self.doc.can_redo();
        if ui
            .add_enabled(can_redo, egui::Button::new("Redo (Ctrl+Shift+Z)"))
            .clicked()
        {
            self.do_redo();
            ui.close_menu();
        }

        ui.separator();
        let has_sel = !self.sel.is_empty();
        if ui
            .add_enabled(has_sel, egui::Button::new("Cut (Ctrl+X)"))
            .clicked()
        {
            self.do_cut(clipboard);
            ui.close_menu();
        }
        if ui
            .add_enabled(has_sel, egui::Button::new("Copy (Ctrl+C)"))
            .clicked()
        {
            self.do_copy(clipboard);
            ui.close_menu();
        }
        if ui.button("Paste (Ctrl+V)").clicked() {
            self.paste_from(clipboard);
            ui.close_menu();
        }
        if ui.button("Select All (Ctrl+A)").clicked() {
            self.sel.set(0, self.doc.len_chars());
            ui.close_menu();
        }

        ui.separator();
        if ui.button("Find… (Ctrl+F)").clicked() {
            self.open_find();
            ui.close_menu();
        }
        if ui.button("Find Next (Ctrl+G)").clicked() {
            self.find_now(true);
            ui.close_menu();
        }
        if ui.button("Find Previous (Ctrl+Shift+G)").clicked() {
            self.find_now(false);
            ui.close_menu();
        }
        if ui.button("Replace… (Ctrl+H)").clicked() {
            self.open_find();
            ui.close_menu();
        }
    }

    /// View: word wrap, font size, Day/Night, custom background.
    fn view_menu(&mut self, ui: &mut egui::Ui, c: &mut AppCommon) {
        let mut wrap = c.settings.wrap;
        if ui.checkbox(&mut wrap, "Word Wrap").changed() {
            c.settings.wrap = wrap;
            c.settings.save();
        }
        let mut nums = c.settings.line_numbers;
        if ui.checkbox(&mut nums, "Line Numbers").changed() {
            c.settings.line_numbers = nums;
            c.settings.save();
        }

        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Font:");
            if ui
                .add_enabled(c.settings.font_size > 8.0, egui::Button::new("A−"))
                .clicked()
            {
                c.settings.font_size = (c.settings.font_size - 1.0).max(8.0);
                c.settings.save();
            }
            ui.label(format!("{:.0}", c.settings.font_size));
            if ui
                .add_enabled(c.settings.font_size < 32.0, egui::Button::new("A+"))
                .clicked()
            {
                c.settings.font_size = (c.settings.font_size + 1.0).min(32.0);
                c.settings.save();
            }
        });

        ui.separator();
        let mut mode = c.settings.theme_mode;
        let mut changed = false;
        changed |= ui.radio_value(&mut mode, ThemeMode::Day, "Day Colors").changed();
        changed |= ui.radio_value(&mut mode, ThemeMode::Night, "Night Colors").changed();
        if changed && mode != c.settings.theme_mode {
            c.settings.theme_mode = mode;
            c.settings.save();
        }
    }

    /// Window: the list of open documents (the current one marked).
    fn window_menu(&mut self, ui: &mut egui::Ui, c: &mut AppCommon) {
        if c.doc_entries.is_empty() {
            ui.add_enabled(false, egui::Label::new("(no documents)"));
            return;
        }
        for (id, title, _dirty) in &c.doc_entries {
            if Some(*id) == self.wid {
                ui.label(egui::RichText::new(format!("> {title}")).strong());
            } else {
                ui.label(egui::RichText::new(format!("   {title}")));
            }
        }
    }

    /// Help: the About box (crediting the original).
    fn help_menu(&mut self, ui: &mut egui::Ui) {
        if ui.button("About Kanpeki").clicked() {
            self.dialog = Some(Dialog::About);
            ui.close_menu();
        }
    }
}

/// Padding between the panel edge and the text, in points.
const PAD: f32 = 8.0;

/// Width of the scrollbar gutter on the right edge of the editor.
const SB_W: f32 = 12.0;

/// Width of the line-number gutter (0 when the numbers are off).
fn gutter_width(show: bool, total_lines: usize, glyph_w: f32) -> f32 {
    if !show {
        return 0.0;
    }
    let digits = total_lines.max(1).to_string().len().max(2);
    digits as f32 * glyph_w + 10.0
}

/// The text area: after the line-number gutter, before the scrollbar.
fn text_area(rect: Rect, gutter: f32) -> Rect {
    Rect::from_min_max(
        Pos2::new(rect.min.x + gutter, rect.min.y),
        Pos2::new(
            (rect.max.x - SB_W).max(rect.min.x + gutter + 1.0),
            rect.max.y,
        ),
    )
}

/// The visible scrollbar track.
fn scrollbar_track(rect: Rect) -> Rect {
    Rect::from_min_max(
        Pos2::new(rect.max.x - SB_W + 2.0, rect.min.y + PAD),
        Pos2::new(rect.max.x - 2.0, rect.max.y - PAD),
    )
}

impl DocWindow {
    /// Handle mouse, wheel and keyboard input for the editor.
    fn handle_editor_input(
        &mut self,
        c: &mut AppCommon,
        ctx: &egui::Context,
        rect: Rect,
        row_h: f32,
        glyph_w: f32,
        clipboard: &mut arboard::Clipboard,
        rr: &mut RedrawResponse,
    ) {
        let gutter = gutter_width(c.settings.line_numbers, self.doc.len_lines(), glyph_w);
        let text = text_area(rect, gutter);
        let track = scrollbar_track(rect);
        let pos = ctx.input(|i| i.pointer.interact_pos());
        let over_text = pos.map(|p| text.contains(p)).unwrap_or(false);
        let over_sb = pos.map(|p| track.contains(p)).unwrap_or(false);

        let pressed = ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary));
        let released = ctx.input(|i| i.pointer.button_released(egui::PointerButton::Primary));
        let down = ctx.input(|i| i.pointer.button_down(egui::PointerButton::Primary));

        // --- Scrollbar: click the track to jump, drag the thumb to scroll.
        self.sb_hover = over_sb || self.sb_dragging;
        if over_sb && pressed {
            if let Some(p) = pos {
                let (_, thumb_y, thumb_h) = self.scrollbar_geom(rect, track, row_h);
                if p.y >= thumb_y && p.y <= thumb_y + thumb_h {
                    self.sb_dragging = true;
                    self.sb_grab = p.y - thumb_y;
                } else {
                    self.sb_dragging = true;
                    self.sb_grab = thumb_h * 0.5;
                    self.scroll_from_y(p.y - self.sb_grab, rect, track, row_h);
                }
            }
        }
        if self.sb_dragging && down {
            if let Some(p) = pos {
                self.scroll_from_y(p.y - self.sb_grab, rect, track, row_h);
            }
        }
        if released || !down {
            self.sb_dragging = false;
        }

        // --- Mouse in the text: click for the caret, drag to select, double-click a word.
        if over_text {
            let double =
                ctx.input(|i| i.pointer.button_double_clicked(egui::PointerButton::Primary));
            if let Some(p) = pos {
                if pressed {
                    let ch = self.char_at(text, p, row_h, glyph_w);
                    if double {
                        let (s, e) = editor::word_range(&self.doc, ch);
                        self.sel.set(s, e);
                        self.pending_scroll = Some(s);
                    } else {
                        self.sel.collapse(ch);
                        self.dragging = true;
                    }
                } else if self.dragging && down {
                    self.sel.head = self.char_at(text, p, row_h, glyph_w);
                }
            }
        }
        if released || !down {
            self.dragging = false;
        }

        // --- Wheel: inertial scrolling (the animation continues in `redraw`).
        if over_text || over_sb {
            let delta = ctx.input(|i| i.smooth_scroll_delta);
            if delta.y != 0.0 {
                self.scroll.nudge(-delta.y * 0.25);
            }
            if delta.x != 0.0 && !c.settings.wrap {
                self.scroll_x = (self.scroll_x - delta.x * 0.25).max(0.0);
            }
        }

        // --- Keyboard.
        let events: Vec<egui::Event> = ctx.input(|i| i.events.iter().cloned().collect());
        for ev in events {
            match ev {
                egui::Event::Text(s) => {
                    if !s.is_empty() {
                        let clean = s.replace("\r\n", "\n").replace('\r', "\n");
                        self.edit_insert(&clean);
                    }
                }
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => {
                    let cmd = modifiers.ctrl || modifiers.command;
                    let handled = if cmd {
                        self.key_shortcut(key, modifiers.shift, c, clipboard, rr)
                    } else {
                        self.key_plain(key, modifiers.shift, rect.height(), row_h)
                    };
                    if handled {
                        break;
                    }
                }
                _ => {}
            }
        }
    }

    /// While a modal is up, Escape dismisses it.
    fn handle_modal_keys(&mut self, ctx: &egui::Context) {
        let events: Vec<egui::Event> = ctx.input(|i| i.events.iter().cloned().collect());
        for ev in events {
            if let egui::Event::Key {
                key: Key::Escape,
                pressed: true,
                ..
            } = ev
            {
                if self.close_prompt {
                    self.close_prompt = false;
                } else {
                    self.dialog = None;
                }
                break;
            }
        }
    }

    /// The character index under a screen position in the editor.
    fn char_at(&self, rect: Rect, pos: Pos2, row_h: f32, glyph_w: f32) -> usize {
        if self.map.lines.is_empty() {
            return 0;
        }
        let y = (pos.y - rect.min.y - PAD + self.scroll.offset).max(0.0);
        let vl = ((y / row_h.max(1.0)) as usize).min(self.map.lines.len() - 1);
        let x = (pos.x - rect.min.x - PAD + self.scroll_x).max(0.0);
        let col = (x / glyph_w.max(1.0)) as usize;
        editor::char_at_visual(&self.doc, &self.map, vl, col)
    }

    /// Keyboard shortcuts with the command (Ctrl) modifier. Returns true if used.
    fn key_shortcut(
        &mut self,
        key: Key,
        shift: bool,
        c: &mut AppCommon,
        clipboard: &mut arboard::Clipboard,
        rr: &mut RedrawResponse,
    ) -> bool {
        match (key, shift) {
            (Key::N, _) => {
                rr.new_windows.push(DocWindow::request(Doc::blank(), c));
                true
            }
            (Key::O, _) => {
                self.begin_open();
                true
            }
            (Key::S, false) => {
                self.save_current(c, rr);
                true
            }
            (Key::S, true) => {
                self.begin_save();
                true
            }
            (Key::W, _) => {
                self.begin_close(rr);
                true
            }
            (Key::Z, false) => {
                self.do_undo();
                true
            }
            (Key::Z, true) | (Key::Y, _) => {
                self.do_redo();
                true
            }
            (Key::F, _) => {
                self.open_find();
                true
            }
            (Key::G, false) => {
                self.find_now(true);
                true
            }
            (Key::G, true) => {
                self.find_now(false);
                true
            }
            (Key::H, _) => {
                self.open_find();
                true
            }
            (Key::A, _) => {
                self.sel.set(0, self.doc.len_chars());
                true
            }
            (Key::X, _) => {
                self.do_cut(clipboard);
                true
            }
            (Key::C, _) => {
                self.do_copy(clipboard);
                true
            }
            (Key::V, _) => {
                self.paste_from(clipboard);
                true
            }
            _ => false,
        }
    }

    /// Unmodified keys (typing, navigation, deletion). Returns true if used.
    fn key_plain(&mut self, key: Key, shift: bool, view_h: f32, row_h: f32) -> bool {
        match key {
            Key::Enter => {
                self.edit_insert("\n");
                true
            }
            Key::Tab => {
                self.edit_insert("\t");
                true
            }
            Key::Backspace => {
                self.delete_back();
                true
            }
            Key::Delete => {
                self.delete_forward();
                true
            }
            Key::ArrowLeft => {
                let pos = self.sel.head.saturating_sub(1);
                if shift {
                    self.sel.head = pos;
                } else {
                    self.sel.collapse(pos);
                }
                self.pending_scroll = Some(pos);
                true
            }
            Key::ArrowRight => {
                let pos = (self.sel.head + 1).min(self.doc.len_chars());
                if shift {
                    self.sel.head = pos;
                } else {
                    self.sel.collapse(pos);
                }
                self.pending_scroll = Some(pos);
                true
            }
            Key::ArrowUp => {
                self.move_visual(-1, shift);
                true
            }
            Key::ArrowDown => {
                self.move_visual(1, shift);
                true
            }
            Key::Home => {
                let pos = self.doc.line_start(self.doc.char_to_line(self.sel.head));
                if shift {
                    self.sel.head = pos;
                } else {
                    self.sel.collapse(pos);
                }
                self.pending_scroll = Some(pos);
                true
            }
            Key::End => {
                let line = self.doc.char_to_line(self.sel.head);
                let pos = self.doc.line_start(line) + self.doc.line_text(line).chars().count();
                if shift {
                    self.sel.head = pos;
                } else {
                    self.sel.collapse(pos);
                }
                self.pending_scroll = Some(pos);
                true
            }
            Key::PageUp => {
                self.page_scroll(-1.0, view_h, row_h);
                true
            }
            Key::PageDown => {
                self.page_scroll(1.0, view_h, row_h);
                true
            }
            Key::Escape => {
                if self.sel.is_empty() {
                    false
                } else {
                    self.sel.collapse(self.sel.head);
                    true
                }
            }
            _ => false,
        }
    }

    /// Move the caret one visual line up/down, keeping its column.
    fn move_visual(&mut self, dir: i32, shift: bool) {
        if self.map.lines.is_empty() {
            return;
        }
        let vl = editor::visual_of_char(&self.map, self.sel.head);
        let col = editor::visual_col_of(&self.doc, &self.map, vl, self.sel.head);
        let n = self.map.lines.len();
        let nv = (vl as i32 + dir).clamp(0, (n - 1) as i32) as usize;
        let ch = editor::char_at_visual(&self.doc, &self.map, nv, col);
        if shift {
            self.sel.head = ch;
        } else {
            self.sel.collapse(ch);
        }
        self.pending_scroll = Some(ch);
    }

    /// Scroll and move the caret one screenful.
    fn page_scroll(&mut self, dir: f32, view_h: f32, row_h: f32) {
        let page = (view_h - 2.0 * row_h).max(row_h);
        let total = self.map.lines.len() as f32 * row_h + 2.0 * PAD;
        let max = (total - view_h).max(0.0);
        self.scroll.velocity = 0.0;
        self.scroll.offset = (self.scroll.offset + dir * page).clamp(0.0, max);

        if !self.map.lines.is_empty() {
            let lines = (page / row_h.max(1.0)).round() as i32;
            let vl = editor::visual_of_char(&self.map, self.sel.head);
            let col = editor::visual_col_of(&self.doc, &self.map, vl, self.sel.head);
            let n = self.map.lines.len() as i32;
            let nv = (vl as i32 + dir.signum() as i32 * lines).clamp(0, n - 1) as usize;
            let ch = editor::char_at_visual(&self.doc, &self.map, nv, col);
            self.sel.collapse(ch);
        }
        self.pending_scroll = None;
    }

    /// Insert text, replacing the selection. The caret ends after the text.
    fn edit_insert(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.find_target = None;
        let (a, b) = self.sel.range();
        if a != b {
            self.doc.replace_range(a, b, text);
        } else {
            self.doc.insert(self.sel.head, text);
        }
        let pos = a + text.chars().count();
        self.sel.collapse(pos);
        self.pending_scroll = Some(pos);
    }

    /// Backspace: delete the selection, or the character before the caret.
    fn delete_back(&mut self) {
        self.find_target = None;
        let (a, b) = self.sel.range();
        if a != b {
            self.doc.delete(a, b);
            self.sel.collapse(a);
            self.pending_scroll = Some(a);
        } else if self.sel.head > 0 {
            let p = self.sel.head - 1;
            self.doc.delete(p, self.sel.head);
            self.sel.collapse(p);
            self.pending_scroll = Some(p);
        }
    }

    /// Delete key: delete the selection, or the character under the caret.
    fn delete_forward(&mut self) {
        self.find_target = None;
        let (a, b) = self.sel.range();
        if a != b {
            self.doc.delete(a, b);
            self.sel.collapse(a);
            self.pending_scroll = Some(a);
        } else if self.sel.head < self.doc.len_chars() {
            self.doc.delete(self.sel.head, self.sel.head + 1);
            self.pending_scroll = Some(self.sel.head);
        }
    }

    /// Undo, restoring the caret.
    fn do_undo(&mut self) {
        if let Some(pos) = self.doc.undo() {
            self.find_target = None;
            self.sel.collapse(pos);
            self.pending_scroll = Some(pos);
            self.note = Some("Undo.".to_string());
        }
    }

    /// Redo, restoring the caret.
    fn do_redo(&mut self) {
        if let Some(pos) = self.doc.redo() {
            self.find_target = None;
            self.sel.collapse(pos);
            self.pending_scroll = Some(pos);
            self.note = Some("Redo.".to_string());
        }
    }

    /// The selected text, if the selection is non-empty.
    fn selection_text(&self) -> Option<String> {
        let (a, b) = self.sel.range();
        if a == b {
            None
        } else {
            Some(self.doc.rope.slice(a..b).into())
        }
    }

    /// Copy the selection to the clipboard.
    fn do_copy(&mut self, clipboard: &mut arboard::Clipboard) {
        if let Some(t) = self.selection_text() {
            let n = t.chars().count();
            let _ = clipboard.set_text(t);
            self.note = Some(format!("Copied {n} characters."));
        }
    }

    /// Cut the selection.
    fn do_cut(&mut self, clipboard: &mut arboard::Clipboard) {
        if let Some(t) = self.selection_text() {
            let n = t.chars().count();
            let _ = clipboard.set_text(t);
            self.find_target = None;
            let (a, b) = self.sel.range();
            self.doc.delete(a, b);
            self.sel.collapse(a);
            self.pending_scroll = Some(a);
            self.note = Some(format!("Cut {n} characters."));
        }
    }

    /// Paste the clipboard at the caret.
    fn paste_from(&mut self, clipboard: &mut arboard::Clipboard) {
        if let Ok(t) = clipboard.get_text() {
            let clean = t.replace("\r\n", "\n").replace('\r', "\n");
            if !clean.is_empty() {
                let n = clean.chars().count();
                self.edit_insert(&clean);
                self.note = Some(format!("Pasted {n} characters."));
            }
        }
    }

    /// Find from the current selection. On a hit, select it and queue a scroll.
    fn do_find(&mut self, forward: bool) -> Result<Option<(usize, usize)>, String> {
        let from = if forward {
            self.sel.range().1
        } else {
            self.sel.range().0
        };
        let hit = self.find.find_next(&self.doc, from, forward)?;
        if let Some((s, e)) = hit {
            self.sel.set(s, e);
            self.find_target = Some((s, e));
            self.pending_scroll = Some(s);
        }
        Ok(hit)
    }

    /// Find with the remembered settings (menu / Ctrl+G without the dialog).
    fn find_now(&mut self, forward: bool) {
        if self.find.query.is_empty() {
            self.open_find();
            return;
        }
        match self.do_find(forward) {
            Ok(Some(_)) => self.note = None,
            Ok(None) => self.note = Some("No matches found.".to_string()),
            Err(e) => self.note = Some(e),
        }
    }

    /// Replace the current match (finding one first if needed).
    fn do_replace(&mut self) -> Result<String, String> {
        let target = match self.find_target {
            Some(t) => t,
            None => match self.do_find(true)? {
                Some(t) => t,
                None => return Ok("No matches found.".to_string()),
            },
        };
        let (s, e) = target;
        let rep = self.find.replacement.clone();
        self.doc.replace_range(s, e, &rep);
        let caret = s + rep.chars().count();
        self.sel.collapse(caret);
        self.pending_scroll = Some(s);
        self.find_target = None;
        // Advance to the next match.
        match self.do_find(true) {
            Ok(Some(_)) => Ok("Replaced 1 match.".to_string()),
            Ok(None) => Ok("Replaced 1 match. No more matches.".to_string()),
            Err(_) => Ok("Replaced 1 match.".to_string()),
        }
    }

    /// Replace every match as one undo step.
    fn do_replace_all(&mut self) -> Result<usize, String> {
        let matches = self.find.find_all(&self.doc)?;
        if matches.is_empty() {
            return Ok(0);
        }
        let rep = self.find.replacement.clone();
        let n = self.doc.replace_all(&matches, &rep);
        self.sel.collapse(0);
        self.find_target = None;
        self.pending_scroll = Some(0);
        Ok(n)
    }
}

impl DocWindow {
    /// Rebuild the visual-line table when text, wrap, width or font size changed.
    fn ensure_map(&mut self, wrap: bool, rect_w: f32, glyph_w: f32, font_size: f32) {
        let cols = if wrap {
            ((rect_w - 2.0 * PAD) / glyph_w.max(1.0)).floor().max(1.0) as usize
        } else {
            0
        };
        let key = MapKey {
            edits: self.doc.edits,
            wrap,
            cols,
            font: (font_size * 2.0) as u32,
        };
        if key != self.map_key {
            self.map = editor::build_visual_map(&self.doc, wrap, cols);
            self.map_key = key;
        }
    }

    /// Scroll so that character `ch` (and the caret) is on screen.
    fn scroll_to_char(&mut self, ch: usize, rect: Rect, row_h: f32, glyph_w: f32) {
        if self.map.lines.is_empty() {
            return;
        }
        let vl = editor::visual_of_char(&self.map, ch);
        let col = editor::visual_col_of(&self.doc, &self.map, vl, ch);

        let y = PAD + vl as f32 * row_h;
        let view_h = rect.height();
        if y < self.scroll.offset + PAD {
            self.scroll.offset = (y - PAD).max(0.0);
        } else if y + row_h > self.scroll.offset + view_h - PAD {
            self.scroll.offset = (y + row_h - view_h + PAD).max(0.0);
        }

        let x = PAD + col as f32 * glyph_w;
        let view_w = rect.width();
        if x < self.scroll_x + PAD {
            self.scroll_x = (x - PAD).max(0.0);
        } else if x > self.scroll_x + view_w - PAD * 2.0 {
            self.scroll_x = (x - view_w + PAD * 2.0).max(0.0);
        }
        self.scroll.velocity = 0.0;
    }

    /// Advance the inertial scroll by one frame. True while still gliding.
    /// `dt` keeps the glide the same wall-clock length at any frame rate.
    fn step_scroll(&mut self, view_h: f32, row_h: f32, dt: f32) -> bool {
        let k = (dt * 60.0).clamp(0.05, 2.0);
        if self.scroll.velocity.abs() <= 0.02 {
            return false;
        }
        let total = self.map.lines.len() as f32 * row_h + 2.0 * PAD;
        let max = (total - view_h).max(0.0);
        let next = self.scroll.offset + self.scroll.velocity * k;
        if next <= 0.0 || next >= max {
            self.scroll.offset = next.clamp(0.0, max);
            self.scroll.velocity = 0.0;
            false
        } else {
            self.scroll.offset = next;
            self.scroll.velocity *= 0.80f32.powf(k);
            true
        }
    }

    /// Paint the text area: current-line highlight, selection, text and caret.
    fn draw_editor(
        &mut self,
        ui: &mut egui::Ui,
        theme: &Theme,
        font_id: &FontId,
        row_h: f32,
        glyph_w: f32,
        rect: Rect,
        wrap: bool,
        font_size: f32,
        line_numbers: bool,
    ) {
        let gutter = gutter_width(line_numbers, self.doc.len_lines(), glyph_w);
        let area = text_area(rect, gutter);
        self.ensure_map(wrap, area.width(), glyph_w, font_size);

        // Reveal a caret position queued by an edit or a find.
        if let Some(ch) = self.pending_scroll.take() {
            self.scroll_to_char(ch, area, row_h, glyph_w);
        }

        // Keep the scroll offset inside the content.
        let total = self.map.lines.len() as f32 * row_h + 2.0 * PAD;
        let max_scroll = (total - rect.height()).max(0.0);
        self.scroll.offset = self.scroll.offset.clamp(0.0, max_scroll);

        // Clip the editor's painting to its own panel; otherwise the lines
        // can draw over the menu bar and status bar while scrolling.
        let clipped = ui.painter().with_clip_rect(rect);
        let painter = &clipped;
        if self.map.lines.is_empty() {
            return;
        }

        let off = self.scroll.offset;
        let nlines = self.map.lines.len();
        let first =
            (((off - PAD) / row_h.max(1.0)).floor().max(0.0) as usize).min(nlines - 1);
        let last = ((((off + rect.height()) / row_h.max(1.0)).ceil() as usize) + 1).min(nlines);

        let caret_vl = editor::visual_of_char(&self.map, self.sel.head);
        let caret_row = self.map.lines[caret_vl.min(nlines - 1)].row;

        let (sel_a, sel_b) = self.sel.range();
        let sel_a_vl = editor::visual_of_char(&self.map, sel_a);
        let sel_b_vl = editor::visual_of_char(&self.map, sel_b);

        let mut widest = 0usize;

        for vl in first..last {
            let line = self.map.lines[vl];
            let y = rect.min.y + PAD + vl as f32 * row_h - off;

            // The line the caret is on.
            if line.row == caret_row {
                let hl = Rect::from_min_max(
                    Pos2::new(rect.min.x, y),
                    Pos2::new(area.max.x, y + row_h),
                );
                painter.rect_filled(hl, 0.0, theme.current_line);
            }

            // A line number only on the first visual row of a wrapped line.
            if gutter > 0.0 && (vl == 0 || self.map.lines[vl - 1].row != line.row) {
                painter.text(
                    Pos2::new(rect.min.x + gutter - 7.0, y + row_h * 0.5),
                    Align2::RIGHT_CENTER,
                    (line.row + 1).to_string(),
                    font_id.clone(),
                    theme.gutter_fg,
                );
            }

            let text = self.doc.line_text(line.row);
            let line_start = self.doc.line_start(line.row);
            let co = line.char_start.saturating_sub(line_start);
            let chunk = editor::char_slice(&text, co, co + line.len);
            let chunk_w = editor::visual_width(chunk);
            widest = widest.max(chunk_w);

            // Selection background for this visual line.
            let mut sel_rect = None;
            if sel_a != sel_b && vl >= sel_a_vl && vl <= sel_b_vl {
                let c0 = if vl == sel_a_vl {
                    editor::visual_col_of(&self.doc, &self.map, vl, sel_a)
                } else {
                    0
                };
                let c1 = if vl == sel_b_vl {
                    editor::visual_col_of(&self.doc, &self.map, vl, sel_b)
                } else {
                    chunk_w
                };
                if c1 > c0 {
                    let x0 = area.min.x + PAD + c0 as f32 * glyph_w - self.scroll_x;
                    let x1 = area.min.x + PAD + c1 as f32 * glyph_w - self.scroll_x;
                    let sr = Rect::from_min_max(Pos2::new(x0, y), Pos2::new(x1, y + row_h));
                    painter.rect_filled(sr, 0.0, theme.selection);
                    sel_rect = Some(sr);
                }
            }

            // The text, with tabs expanded so columns match the wrap map.
            if !chunk.is_empty() {
                let mut shown = String::with_capacity(chunk.len());
                for ch in chunk.chars() {
                    if ch == '\t' {
                        shown.push_str("    ");
                    } else {
                        shown.push(ch);
                    }
                }
                let x = area.min.x + PAD - self.scroll_x;
                painter.text(
                    Pos2::new(x, y + row_h * 0.5),
                    Align2::LEFT_CENTER,
                    &shown,
                    font_id.clone(),
                    theme.text_fg,
                );
                // Repaint the selected run in the inverted colour. Clipping to
                // the selection rect keeps the glyph positions identical, so
                // the boundary is exactly the column edge.
                if let Some(sr) = sel_rect {
                    if theme.selection_fg != theme.text_fg {
                        painter.with_clip_rect(sr.intersect(rect)).text(
                            Pos2::new(x, y + row_h * 0.5),
                            Align2::LEFT_CENTER,
                            &shown,
                            font_id.clone(),
                            theme.selection_fg,
                        );
                    }
                }
            }
        }

        // Horizontal bound, including the caret's line so the view doesn't
        // snap back while typing at the end of a long line.
        let caret_line_w = editor::visual_width(&self.doc.line_text(caret_row));
        let widest = widest.max(caret_line_w);
        let max_x = (widest as f32 * glyph_w - area.width() + 2.0 * PAD).max(0.0);
        self.scroll_x = self.scroll_x.clamp(0.0, max_x);

        // The insertion point: a solid bar, in the spirit of System 7.
        let caret_col = editor::visual_col_of(&self.doc, &self.map, caret_vl, self.sel.head);
        let cx = area.min.x + PAD + caret_col as f32 * glyph_w - self.scroll_x;
        let cy = rect.min.y + PAD + caret_vl as f32 * row_h - off;
        let caret = Rect::from_min_size(
            Pos2::new(cx, cy + 1.0),
            egui::vec2(1.5, (row_h - 2.0).max(2.0)),
        );
        painter.rect_filled(caret, 0.0, theme.accent);

        // A hairline between the gutter and the text.
        if gutter > 0.0 {
            let x = rect.min.x + gutter - 1.0;
            painter.rect_filled(
                Rect::from_min_max(Pos2::new(x, rect.min.y), Pos2::new(x + 1.0, rect.max.y)),
                0.0,
                theme.border.gamma_multiply(0.55),
            );
        }

        self.draw_scrollbar(painter, rect, row_h, theme);
    }

    /// (max scroll, thumb top, thumb height) in track coordinates.
    fn scrollbar_geom(&self, rect: Rect, track: Rect, row_h: f32) -> (f32, f32, f32) {
        let total = self.map.lines.len() as f32 * row_h + 2.0 * PAD;
        let max = (total - rect.height()).max(0.0);
        if max <= 0.0 {
            return (0.0, track.min.y, track.height());
        }
        let frac = (rect.height() / total).clamp(0.0, 1.0);
        let thumb_h = (track.height() * frac).max(28.0).min(track.height());
        let t = (self.scroll.offset / max).clamp(0.0, 1.0);
        let y = track.min.y + t * (track.height() - thumb_h);
        (max, y, thumb_h)
    }

    /// Scroll so the thumb's top lands at `y`.
    fn scroll_from_y(&mut self, y: f32, rect: Rect, track: Rect, row_h: f32) {
        let (max, _, thumb_h) = self.scrollbar_geom(rect, track, row_h);
        if max <= 0.0 {
            return;
        }
        let span = (track.height() - thumb_h).max(1.0);
        let t = ((y - track.min.y) / span).clamp(0.0, 1.0);
        self.scroll.offset = t * max;
        self.scroll.velocity = 0.0;
    }

    /// Draw the vertical scrollbar in the right-hand gutter.
    fn draw_scrollbar(&self, painter: &egui::Painter, rect: Rect, row_h: f32, theme: &Theme) {
        let track = scrollbar_track(rect);
        let (max, y, h) = self.scrollbar_geom(rect, track, row_h);
        if max <= 0.0 {
            return;
        }
        painter.rect_filled(track, 4.0, theme.current_line);
        let thumb = Rect::from_min_size(Pos2::new(track.min.x, y), egui::vec2(track.width(), h));
        let color = if self.sb_dragging || self.sb_hover {
            theme.accent
        } else {
            theme.border
        };
        painter.rect_filled(thumb, 4.0, color);
    }

    /// The status bar: name, position, counts, EOL, encoding, notes.
    fn status_bar(&mut self, ui: &mut egui::Ui, c: &mut AppCommon, theme: &Theme) {
        // Pick up finished background word counts.
        while let Ok((gen, words)) = self.words_rx.try_recv() {
            if gen == self.word_gen() {
                self.words = Some(words);
            }
        }
        if self.words_gen != self.word_gen() {
            self.count_words();
        }

        ui.horizontal(|ui| {
            ui.add_space(PAD);
            let name = doc_title(&self.doc);
            let name = if self.doc.modified {
                format!("* {name}")
            } else {
                name
            };
            ui.label(egui::RichText::new(name).strong().color(theme.menu_fg));

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            let line = self.doc.char_to_line(self.sel.head) + 1;
            let col = self.sel.head - self.doc.line_start(line - 1) + 1;
            ui.label(format!("Ln {line}, Col {col}"));

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);
            let bytes = self.doc.len_bytes();
            let words = match self.words {
                Some(w) => w.to_string(),
                None => "…".to_string(),
            };
            ui.label(format!("{bytes} bytes · {words} words"));

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);
            ui.label(self.doc.eol.label());
            ui.add_space(6.0);
            ui.label(self.doc.encoding.label());

            if let Some(note) = &self.note {
                ui.add_space(12.0);
                ui.label(egui::RichText::new(note).weak());
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(PAD);
                ui.label(
                    egui::RichText::new(if c.settings.wrap { "wrap" } else { "no wrap" }).weak(),
                );
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(format!("{:.0} pt", c.settings.font_size)).weak(),
                );
            });
        });
    }

    /// A generation that changes on every edit *and* every document load.
    fn word_gen(&self) -> u64 {
        self.load_seq.wrapping_shl(32) ^ (self.doc.edits & 0xffff_ffff)
    }

    /// Count words: synchronously for small files, on a worker thread for big ones.
    fn count_words(&mut self) {
        let gen = self.word_gen();
        if self.words_gen == gen {
            return;
        }
        self.words_gen = gen;
        if self.doc.len_bytes() < 1_000_000 {
            self.words = Some(self.doc.word_count());
            return;
        }
        let rope = self.doc.rope.clone();
        let tx = self.words_tx.clone();
        std::thread::spawn(move || {
            let mut words = 0usize;
            let mut in_word = false;
            for ch in rope.chars() {
                if ch.is_whitespace() {
                    in_word = false;
                } else if !in_word {
                    in_word = true;
                    words += 1;
                }
            }
            let _ = tx.send((gen, words));
        });
    }

    /// The unsaved-changes prompt. `None` while no button has been pressed.
    fn close_dialog(&self, ctx: &egui::Context, theme: &Theme) -> Option<CloseAct> {
        let screen = ctx.screen_rect();
        let mut act = None;
        egui::Window::new("close_prompt")
            .id(egui::Id::new("close_prompt"))
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .movable(false)
            .fixed_pos(screen.center() - egui::vec2(230.0, 60.0))
            .frame(egui::Frame::none())
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(theme.dialog_bg)
                    .stroke(egui::Stroke::new(1.0_f32, theme.border))
                    .rounding(4.0)
                    .inner_margin(egui::Margin::same(14.0))
                    .show(ui, |ui| {
                        ui.set_width(420.0);
                        ui.strong(egui::RichText::new("Unsaved Changes").size(15.0));
                        ui.add_space(8.0);
                        ui.label(format!(
                            "{} has unsaved changes.\nSave them before closing?",
                            self.title_string()
                        ));
                        ui.add_space(12.0);
                        ui.allocate_ui_with_layout(
                            egui::vec2(ui.available_width(), 26.0),
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                if ui.button("Save").clicked() {
                                    act = Some(CloseAct::Save);
                                }
                                if ui.button("Don't Save").clicked() {
                                    act = Some(CloseAct::Discard);
                                }
                                if ui.button("Cancel").clicked() {
                                    act = Some(CloseAct::Cancel);
                                }
                            },
                        );
                    });
            });
        act
    }

    /// Act on the unsaved-changes prompt.
    fn do_close_act(&mut self, act: CloseAct, c: &mut AppCommon, rr: &mut RedrawResponse) {
        match act {
            CloseAct::Cancel => {
                self.close_prompt = false;
            }
            CloseAct::Discard => {
                self.close_prompt = false;
                self.force_close = true;
                rr.quit = true;
            }
            CloseAct::Save => {
                self.close_prompt = false;
                if let Some(path) = self.doc.path.clone() {
                    match fileio::save(&self.doc, &path) {
                        Ok(()) => {
                            self.doc.modified = false;
                            self.force_close = true;
                            rr.quit = true;
                            if let Some(s) = path.to_str() {
                                c.settings.add_recent(s);
                                c.settings.save();
                            }
                        }
                        Err(e) => self.note = Some(e),
                    }
                } else {
                    // Untitled: ask for a name, then close once saved.
                    self.save_then_close = true;
                    self.begin_save();
                }
            }
        }
    }

    /// Apply an action chosen in a dialog.
    fn apply_action(&mut self, act: Action, c: &mut AppCommon, rr: &mut RedrawResponse) {
        match act {
            Action::None => {}
            Action::Close => {
                // Keep any edits the user made in the find panel.
                if let Some(Dialog::Find { search, .. }) = &self.dialog {
                    self.find = search.clone();
                }
                self.dialog = None;
            }
            Action::OpenFile(path) => {
                self.open_path(path, c, rr);
            }
            Action::SaveFile(path) => {
                self.dialog = None;
                self.save_to(path, c, rr);
            }
            Action::FindNext(forward) => {
                if let Some(Dialog::Find { search, .. }) = &self.dialog {
                    self.find = search.clone();
                }
                let err = match self.do_find(forward) {
                    Ok(Some(_)) => None,
                    Ok(None) => Some("No matches found.".to_string()),
                    Err(e) => Some(e),
                };
                if let Some(Dialog::Find { error, .. }) = &mut self.dialog {
                    *error = err;
                }
            }
            Action::Replace => {
                if let Some(Dialog::Find { search, .. }) = &self.dialog {
                    self.find = search.clone();
                }
                let msg = match self.do_replace() {
                    Ok(m) => m,
                    Err(e) => e,
                };
                if let Some(Dialog::Find { error, .. }) = &mut self.dialog {
                    *error = Some(msg);
                }
            }
            Action::ReplaceAll => {
                if let Some(Dialog::Find { search, .. }) = &self.dialog {
                    self.find = search.clone();
                }
                let msg = match self.do_replace_all() {
                    Ok(0) => "No matches found.".to_string(),
                    Ok(n) => {
                        self.note = Some(format!("Replaced {n} matches."));
                        format!("Replaced {n} matches.")
                    }
                    Err(e) => e,
                };
                if let Some(Dialog::Find { error, .. }) = &mut self.dialog {
                    *error = Some(msg);
                }
            }
        }
    }
}




