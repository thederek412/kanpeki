# Kanpeki (Rust) — Project Handoff

**Date:** 2026-11-14
**Project:** `/run/media/derekk/USB/kanpeki`
**Author:** Derek Kalinosky
**Goal:** **Kanpeki** — a fast, multi-window native text editor in the spirit of the classic System 7 editor **SaintEdit** (Craig Marciniak, 1993) for Manjaro/X11: multiple true OS windows (one per document), deep undo, atomic safe saves, find & replace, Day/Night palettes, FastScrolls-style smooth scrolling, line-ending auto-detect/convert (Unix/DOS/classic Mac), large-file support, status bar, About box crediting the original.

**No speech/TTS, no printing** (user explicitly excluded).

---

## Current Status

| Item | State |
|---|---|
| Toolchain | Rust stable via rustup (`source ~/.cargo/env`) |
| `Cargo.toml` | Final dependency set ✔ |
| `src/theme.rs` | Complete ✔ |
| `src/buffer.rs` | Complete ✔ |
| `src/fileio.rs` | Complete ✔ |
| `src/search.rs` | Complete ✔ |
| `src/editor.rs` | Complete ✔ |
| `src/dialogs.rs` | Complete ✔ |
| `src/windows.rs` | **Complete and tested** (TrackedWindow, menus, input/editing/find, drawing, status bar, close prompt) |
| `src/main.rs` | **Written and building** (code below) |
| `docs/` | README screenshots (Day, Night, Find & Replace), captured from the release build |
| `src/icon.rs` | Complete ✔ — embeds the shiba inu icon and decodes it for each window |
| `src/fonts.rs` | Complete ✔ — embeds Hurmit Nerd Font Mono and registers it as `kanpeki-mono` |
| `assets/` | Icons, plus `fonts/HurmitNerdFontMono-Regular.otf` (SIL OFL) and its license |
| Build | **Clean** — `cargo build` succeeds, 3 benign warnings (unused macro import, unused `Scroll::step/clamp`, unused `Theme::chrome`) |

Already fixed while writing Parts 2–5 (avoid re-introducing these):
- ropey's `get_line` includes the trailing `\n`; `Doc::line_text` now strips it.
- egui 0.27 has **no** `Frame::NONE` (use `Frame::none()`) and **no** `StrokeKind` (pass the stroke directly).
- `theme.rs` and `dialogs.rs` needed `use egui_multiwin::egui;`.
- Dialog boxes now use a content-sized `Frame` (painting `ui.max_rect()` inside an `Area` fills the screen).
- `Theme::resolve` now derives text/line colors from the *resolved* background: Day mode was rendering near-white text on white.
- Dialog × buttons were dead (body action overwrote them); the × was then removed in favour of Close/Cancel.
- Open and Save use the system picker (`zenity`, background thread + channel polled from `redraw`); in-app dialogs remain as fallbacks.

### Verified on this machine (X11, 4480x1440)

- Multi-window (Ctrl+N), cascaded windows, Window-menu document list, per-window titles with `*` dirty marker.
- Typing, caret, click/drag selection, double-click word select; status bar Ln/Col, bytes, words, EOL, encoding, notes.
- Save (atomic), Save As with full paths, recents; title/status update after save.
- Undo/redo (Ctrl+Z / Ctrl+Shift+Z / Ctrl+Y), find & replace (Next/Previous/Replace/Replace All with messages).
- Word wrap (reflows long lines) and Day/Night palettes (persisted in `settings.json`). The palettes are a duotone: Day is night ink (`#32364a`) on day paper (`#9399b7`); Night is day paper on night ink. Night's selection inverts the pair (paper band, ink text).
- EOL round-trips: CRLF file stayed CRLF, classic-Mac CR stayed CR, UTF-8 BOM preserved; each detected correctly.
- 8.96 MB file: opens in about a second, wheel scrolling with inertia, typing stays instant (word count off-thread).
- Unsaved-changes prompt on Ctrl+W / File > Close; Cancel / Don't Save / Save; app exits when the last window closes.
- About box credits Derek Kalinosky (author) and Craig Marciniak/SaintEdit (inspiration); closes via Close or Esc.
- File > Open (and Ctrl+O) launches the system file picker; the chosen file loads in place or in a new window.
- Save (Ctrl+S on an untitled document), Save As (Ctrl+Shift+S) and the close prompt's Save use the system save dialog; cancelling it leaves the document and window alone.
- Vertical scrollbar appears only when the document is longer than the view; click-to-jump, draggable thumb, hover highlight.
- Editor painting is clipped to its panel, so scrolled lines never draw over the menu bar or status bar.
- Optional line-number gutter (View > Line Numbers), persisted; wrapped lines are numbered only on their first visual row.
- The custom-background option was removed: it fought the Day/Night palettes (and the derived current-line/selection colours). Existing settings files that still contain `custom_bg` load fine; the field is ignored.
- Find & Replace is a tidied modal: centered, accent title rule, full-width fields with hints, uppercase captions, a primary *Next* button, and a status line that distinguishes info (muted), success (green) and errors (red). The query field is focused (and selected) on open; Enter/Shift+Enter find next/previous; actions are disabled for an empty query; the query keeps keyboard focus after Find/Replace.

### Known environment quirks

- **Window focus (fixed in-app):** winit 0.29 selects XInput2 `FocusIn` on a window just after
  mapping it. If the WM focuses the window during that map, the event is missed, `active_window`
  stays `None`, and `xinput_key_input` silently returns: all typing and shortcuts die while the
  mouse still works. Kanpeki now detects `!window.has_focus()` on the first frame and performs a
  one-shot hide/show plus `focus_window()` so the WM delivers `FocusIn` again. Verified: a freshly
  launched window takes typed text and Ctrl+F without any manual focus cycling.
- The WM here adds decoration offsets; `xdotool getwindowgeometry` reports the frame origin.
- `import -window` captures are client-only; add the window origin before comparing with screen clicks.

### Polish added after 1.0.0

- **Menu shortcuts flush right.** Drop-down rows are custom `menu_item` rows
  (`MENU_W = 250`, `MENU_ROW_H = 21`): the label sits left, the shortcut right,
  and disabled rows are weak/inert. Native `ui.button`s remain for the Open
  Recent submenu and the checkboxes in View. The native Open Recent row is
  aligned by temporarily setting `button_padding = (10, 0)` and
  `interact_size.y = MENU_ROW_H` around it, and custom hover fills use the
  same expansion/rounding as egui's native menu buttons.
- **Scrollbars read at a glance.** The thumb is drawn in `theme.accent`
  (dimmed to 60% until hovered/dragged). The old neutral `theme.border` thumb
  was almost the same value as the track in both palettes.
- **Horizontal scrollbar** when word wrap is off and the widest line overflows.
  `VisualMap.max_width` (cells) is computed by `build_visual_map`; the text is
  clipped to the text area so horizontally scrolled lines never paint over the
  line-number gutter. Drag/click the track or use the wheel with Shift.
- **Coalesced undo.** `Doc::push_undo` merges contiguous typing (`Insert` at the
  end of the previous run) and contiguous Backspace/Delete runs into one step,
  capped at `UNDO_GROUP_CAP` (4000 chars).
- **Ctrl+Home / Ctrl+End** go to the document start/end (Shift extends).
- **Triple-click** selects the current line (newline excluded).
- **Drag auto-scroll**: dragging a selection past an edge scrolls and keeps
  extending until the button is released (`redraw_again` requests frames).
- **Overwrite confirmation** in the built-in Save dialog: a second Save (same
  path) confirms. The system picker keeps GTK's own confirmation; the
  `--confirm-overwrite` flag is passed but is deprecated/no-op on current zenity.
- **Command-line path** opens in the first window (`kanpeki FILE`).
- **Window geometry is remembered** across runs (`window_size` / `window_pos`
  in settings.json). It is written once the window has been still for 0.75 s,
  and again on close, so dragging does not hammer the settings file.

Note (egui 0.27): `button_double_clicked` / `button_triple_clicked` are true on
**release**, not on press. Kanpeki applies word/line selection on the release
frame; handling them inside the `pressed` branch silently breaks double-click
unless press and release land in the same frame.

---

## Build & Run

```bash
source ~/.cargo/env
cd /run/media/derekk/USB/kanpeki
cargo build            # debug
cargo build --release  # lto + stripped

DISPLAY=:0.0 ./target/debug/kanpeki
cargo build --release
DISPLAY=:0.0 ./target/release/kanpeki
```

A ready-to-run release binary is also at `./kanpeki` (7.1 MB, stripped + LTO).

Notes:
- The project lives on the USB drive (exFAT-like filesystem: directory size 131072, all files show `rwxr-xr-x`). Builds there work but may be slow. If compile times are painful:
  `CARGO_TARGET_DIR=/tmp/opencode/saintedit-target cargo build`
- X11 session on `DISPLAY=:0.0`. Screenshot verification:
  `DISPLAY=:0.0 import -window root /tmp/opencode/shot.png`
- No git repo is initialized.

---

## Dependencies (`Cargo.toml`)

```toml
egui-multiwin = "0.5"   # resolves 0.5.2 → pins egui 0.27.2 / winit 0.29.15 transitively
ropey = "1.6"
regex = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
dirs = "6"
png = "0.17"          # decode the embedded window icon

[profile.release]
lto = true
strip = true
```

**Do not add eframe** — it has no multi-window API. `egui-multiwin` is the only working path.
Never depend on egui directly; always use `egui_multiwin::egui` (it re-exports 0.27.2).

---

## Architecture

```
main.rs      — macro wiring, AppCommon (shared state), CustomEvent, Windows enum, main()
windows.rs   — DocWindow: the whole per-document window (menu, editor, status bar, dialogs)
buffer.rs    — Doc: ropey rope + EOL/encoding metadata + undo/redo op stack
editor.rs    — visual-line map (word wrap), smooth Scroll, Selection, hit-test helpers
fileio.rs    — decode (UTF-8/BOM/UTF-16/Latin-1), EOL detect, atomic save, dir listing
search.rs    — Search settings + find_all / find_next (plain, regex, case, whole word)
dialogs.rs   — in-window modal dialogs: Open, Save, Find, Color, About → Action
theme.rs     — Settings (persisted), Theme palettes (Day/Night), egui visuals
fonts.rs     — bundled Hurmit Nerd Font Mono, registered under the `kanpeki-mono` family
```

### Editor typeface

The editor does not depend on any font being installed. `fonts::bundled()`
`include_bytes!`s `assets/fonts/HurmitNerdFontMono-Regular.otf` into the binary;
`main` registers it with `mw.add_font(fonts::FAMILY, …)` **before** adding the
first window, and `windows.rs` builds the editor `FontId` with
`FontFamily::Name("kanpeki-mono")`. The status bar shows the face name. The
font is SIL OFL 1.1 (Hermit by Pablo Caro, Nerd-Fonts patched); the license
travels in `assets/fonts/`.

### Data-flow decisions (already locked in)

- Text is **always stored with `\n`** in the rope. `Doc.eol` is detected on load and re-applied on save (`fileio::render_for_save`).
- Undo is **op-based** (`Insert` / `Delete` / `Replace` / `ReplaceAll`), cap 400, and
  contiguous typing/deleting is coalesced into groups (cap 4000 chars).
  `Doc::undo()` / `Doc::redo()` return `Option<usize>` = caret position to restore.
- `Doc.edits: u64` bumps on every change; used as the visual-map cache key.
- Save is atomic: temp file in the same dir → `write_all` → `sync_all` → `rename` → best-effort dir fsync.
- Settings persist to `~/.config/kanpeki/settings.json` (migrates the old `saintedit` file once) (recents capped at 8).
- Dialogs are **egui `Area`s with `Order::Foreground`** inside the same OS window, not separate windows.

---

## Public API of the finished modules (for the remaining code)

### buffer.rs

```rust
pub enum Eol { Unix, Dos, Classic }          // .bytes(), .label() -> "LF"|"CRLF"|"CR"
pub enum Encoding { Utf8, Utf8Bom, Utf16, Latin1 }  // .label()

pub struct Doc {
    pub rope: ropey::Rope,
    pub path: Option<std::path::PathBuf>,
    pub modified: bool,
    pub eol: Eol,
    pub encoding: Encoding,
    pub word_cache: Option<usize>,
    pub edits: u64,
    // undo/redo: private
}

impl Doc {
    pub fn blank() -> Self;
    pub fn from_text(text: &str, eol: Eol, encoding: Encoding, path: Option<PathBuf>) -> Self; // normalizes CRLF/CR -> \n
    pub fn len_chars(&self) -> usize;
    pub fn len_bytes(&self) -> usize;
    pub fn len_lines(&self) -> usize;
    pub fn get_char(&self, idx: usize) -> Option<char>;
    pub fn char_to_line(&self, idx: usize) -> usize;
    pub fn line_start(&self, line: usize) -> usize;      // clamps to len_chars()
    pub fn line_text(&self, line: usize) -> String;      // uses Rope::get_line (verified to exist)
    pub fn to_string(&self) -> String;
    pub fn insert(&mut self, at: usize, text: &str);
    pub fn delete(&mut self, start: usize, end: usize);
    pub fn replace_range(&mut self, start: usize, end: usize, text: &str); // single undo step
    pub fn replace_all(&mut self, matches: &[(usize, usize)], replacement: &str) -> usize; // one undo step
    pub fn can_undo(&self) -> bool;
    pub fn can_redo(&self) -> bool;
    pub fn undo(&mut self) -> Option<usize>;   // Some(caret)
    pub fn redo(&mut self) -> Option<usize>;
    pub fn word_count(&mut self) -> usize;     // synchronous, cached — prefer the background counter in DocWindow
}
```

### editor.rs

```rust
pub struct VisualLine { pub char_start: usize, pub row: usize, pub len: usize }
pub struct VisualMap {
    pub lines: Vec<VisualLine>,
    pub max_width: usize,             // widest line in cells (0 when wrapped)
}                                    // construct with VisualMap { lines: Vec::new(), max_width: 0 }

pub fn visual_width(s: &str) -> usize;                            // tab = 4 cells
pub fn build_visual_map(doc: &Doc, wrap: bool, cols: usize) -> VisualMap; // O(doc)
pub fn visual_of_char(map: &VisualMap, idx: usize) -> usize;
pub fn char_at_visual(doc: &Doc, map: &VisualMap, vl: usize, col: usize) -> usize;
pub fn visual_col_of(doc: &Doc, map: &VisualMap, vl: usize, char_idx: usize) -> usize;
pub fn char_slice(s: &str, a: usize, b: usize) -> &str;            // char-index safe
pub fn word_range(doc: &Doc, idx: usize) -> (usize, usize);

pub struct Scroll { pub offset: f32, pub velocity: f32 }
impl Scroll { pub fn new() -> Self; pub fn nudge(&mut self, delta: f32); pub fn step(&mut self) -> bool; pub fn clamp(&mut self, max: f32); }

pub struct Selection { pub anchor: usize, pub head: usize }  // head == caret
impl Selection { pub fn range(&self) -> (usize, usize); pub fn is_empty(&self) -> bool; pub fn set(&mut self, a, h); pub fn collapse(&mut self, pos); }
```

`VisualLine.char_start` is an absolute char index. To slice a visual line's text out of the full document line, use
`co = char_start - doc.line_start(row)` then `char_slice(&doc.line_text(row), co, co + len)`.

### fileio.rs

```rust
pub struct Loaded { pub text: String, pub eol: Eol, pub encoding: Encoding }
pub fn load(path: &Path) -> Result<Loaded, String>;
pub fn render_for_save(doc: &Doc) -> Vec<u8>;
pub fn save(doc: &Doc, path: &Path) -> Result<(), String>;   // atomic
pub fn default_dir() -> PathBuf;                              // home
pub fn dir_entries(dir: &Path) -> (Vec<String> /*dirs*/, Vec<String> /*files*/);
```

### search.rs

```rust
#[derive(Clone, Default)]
pub struct Search {
    pub query: String,
    pub replacement: String,
    pub case_sensitive: bool,
    pub regex: bool,
    pub whole_word: bool,
}
impl Search {
    pub fn compile(&self) -> Result<regex::Regex, String>;
    pub fn find_all(&self, doc: &Doc) -> Result<Vec<(usize, usize)>, String>;
    /// forward: first match with end > from, wraps. backward: last match with start < from, wraps.
    pub fn find_next(&self, doc: &Doc, from: usize, forward: bool) -> Result<Option<(usize, usize)>, String>;
}
```

Note: despite the name, `compile()` returns `Result<Regex, String>`.

### theme.rs

```rust
pub enum ThemeMode { Day, Night }
pub struct Settings {
    pub font_size: f32,          // default 15.0
    pub theme_mode: ThemeMode,
    pub wrap: bool,
    #[serde(default)]
    pub line_numbers: bool,
    pub recents: Vec<String>,
    #[serde(default)]
    pub window_size: Option<[f32; 2]>,  // physical pixels
    #[serde(default)]
    pub window_pos: Option<[f32; 2]>,   // outer position
}
impl Settings { pub fn load() -> Self; pub fn save(&self); pub fn add_recent(&mut self, path: &str); }

#[derive(Clone, Copy)]
pub struct Theme {
    pub mode: ThemeMode,
    pub chrome, menu_bar, menu_fg, text_bg, text_fg, accent,
    pub selection, selection_fg, widget_selection,
    pub gutter_fg, hint_fg, caption_fg,
    pub current_line, status_bg, dialog_bg, border, widget_fill, widget_hover: egui::Color32,
}
impl Theme { pub fn resolve(settings: &Settings) -> Self; pub fn egui_visuals(&self) -> egui::Visuals; }
```

### dialogs.rs

```rust
pub enum Dialog {
    Open(OpenDlg),
    Save(SaveDlg),
    Find { search: Search, error: Option<String> },
    Color { r: u8, g: u8, b: u8 },
    About,
}
pub struct OpenDlg { pub dir: PathBuf, pub name: String, pub error: Option<String> }
pub struct SaveDlg { pub dir: PathBuf, pub name: String, pub error: Option<String> }

pub enum Action {
    None, Close,
    OpenFile(PathBuf), SaveFile(PathBuf),
    FindNext(bool),                 // true = forward
    Replace, ReplaceAll,
    ApplyColor(u8, u8, u8),
}

pub fn show(ctx: &egui::Context, dlg: &mut Dialog, theme: &Theme) -> Action;
```

Dialogs are `egui::Window`s anchored `CENTER_CENTER` (so they are truly centred regardless of their auto height) with a soft drop shadow, an accent rule beside the title, and a hairline under it (plain `Area`s did not receive clicks under egui-multiwin). About has a Close button, the other dialogs have Cancel, and Esc dismisses any dialog (no × by design). **Open and Save now use the desktop's file picker (`zenity`)**; the in-app dialogs remain as fallbacks when no picker is installed. The in-app Save dialog accepts an absolute path typed into the name field (anything containing `/`). The system save dialog starts in the document's directory, pre-filled with its file name (just `Untitled` for a new document, so GTK selects only the base name and does not double an extension).

---

## `windows.rs` — design notes (all implemented)

### Core state

- Imports (egui via `egui_multiwin::egui`, `arboard`, `EguiGlow`, winit `Window`/`WindowId`).
- `enum CloseAct { Cancel, Discard, Save }`.
- `struct MapKey { edits, wrap, cols, font }` with `MapKey::INVALID`.
- `struct DocWindow { ... }` — fields:
  `doc, sel: Selection, scroll: Scroll, scroll_x: f32, map: VisualMap, map_key: MapKey,
   dialog: Option<Dialog>, find: Search, find_target: Option<(usize, usize)>,
   close_prompt: bool, force_close: bool, save_then_close: bool,
   wid: Option<WindowId>, last_title: String,
   words: Option<usize>, words_gen: u64, words_tx/words_rx: mpsc channel,
   dragging: bool, note: Option<String>`
- `fn doc_title(&Doc) -> String`, `fn short_name(&str) -> String`.
- `impl DocWindow`:
  - `pub fn new(doc: Doc) -> Self`
  - `pub fn request(doc: Doc, c: &mut AppCommon) -> NewWindowRequest` (cascaded position, 1024×720, title)
  - `fn title_string(&self) -> String` (`● name` when modified)
  - `fn begin_close(&mut self, rr)` (prompt if dirty, else `rr.quit = true`)
  - `fn save_current(&mut self, c, rr)` (save; honors `save_then_close`; falls back to Save dialog)
  - `fn open_save_dialog(&mut self)`
  - `fn open_path(&mut self, path, c, rr)` (loads in place if current window is blank+unmodified, else new window; updates recents)
  - `fn open_find(&mut self)`
- Ends with the sentinel line `// __PART2__`.

### The `TrackedWindow` impl

```rust
fn is_root(&self) -> bool { true }

fn can_quit(&mut self, c: &mut AppCommon) -> bool {
    if self.doc.modified && !self.force_close {
        self.close_prompt = true;   // show the prompt on the next redraw
        false
    } else {
        if let Some(id) = self.wid { c.remove_entry(id); }
        true
    }
}

fn redraw(&mut self, c: &mut AppCommon, egui: &mut EguiGlow, window: &Window,
          clipboard: &mut arboard::Clipboard) -> RedrawResponse
```

`redraw` skeleton (order matters):

1. `let ctx = egui.egui_ctx.clone();`
2. `let theme = Theme::resolve(&c.settings); ctx.set_visuals(theme.egui_visuals());`
3. Bookkeeping: `window.id()` → `self.wid`; `c.upsert_entry(wid, title, modified)`; if `window.has_focus()` set `c.active_window = Some(wid)`; `window.set_title(...)` only when changed (`last_title`).
4. `let modal = self.close_prompt || self.dialog.is_some();`
5. `TopBottomPanel::top("menubar").frame(Frame::NONE.fill(theme.menu_bar)).show(...)` → menu bar.
6. Editor font: `let font_id = FontId::new(c.settings.font_size, FontFamily::Monospace);`
   `let (row_h, glyph_w) = ctx.fonts(|f| (f.row_height(&font_id), f.glyph_width(&font_id, 'x')));`
7. `CentralPanel::default().frame(Frame::NONE.fill(theme.text_bg)).show(...)`:
   `let rect = ui.max_rect(); self.draw_editor(...); if !modal { self.handle_editor_input(...); }`
8. `TopBottomPanel::bottom("statusbar")` → status bar.
9. Dialogs: if `close_prompt` → `close_dialog()` + `do_close_act()`; else if `dialog` → `dialogs::show()` + `apply_action()`.
10. Smooth scroll: `if self.step_scroll(rect.height(), row_h) { window.request_redraw(); }`
11. Return `rr`.

`custom_event` is overridden trivially (the app keeps all communication local to a window).

### Menu bar

`ui.menu_button("File", |ui| ...)` etc. — **no `open` parameter in egui 0.27**; call `ui.close_menu()` after an action. When `modal`, render the five menu names as weak labels instead of real menus (modal menus are disabled).

- File: New (Ctrl+N) → `rr.new_windows.push(DocWindow::request(Doc::blank(), c))`; Open… → `Dialog::Open(OpenDlg { dir: fileio::default_dir(), name: "", error: None })`; Open Recent (submenu over `c.settings.recents.clone()`, label = `short_name`); Close (Ctrl+W) → `begin_close`; Save (Ctrl+S) → `save_current`; Save As… → `open_save_dialog`; Quit Kanpeki → `begin_close` (goes through the unsaved prompt; the app exits when the last window closes).
- Edit: Undo/Redo via `ui.add_enabled(self.doc.can_undo(), egui::Button::new(...))`; Cut/Copy/Paste (need `&mut arboard::Clipboard` threaded into the menu fn); Select All; Find… (Ctrl+F), Find Next (Ctrl+G), Find Previous (Ctrl+Shift+G), Replace… (Ctrl+H) — both Find and Replace open `open_find()`; Count Words → `self.count_words()` (background thread).
- View: `ui.checkbox(&mut c.settings.wrap, "Word Wrap")`; font `A−`/`A+` buttons clamped 8.0..=32.0; `ui.radio_value(&mut c.settings.theme_mode, ThemeMode::Day/Night, ...)`. Call `c.settings.save()` after changes.
- Window: informational document list from `c.doc_entries` (`(WindowId, title, dirty)`) — current window marked `▸` + strong, others weak. (Cross-window focus is impossible: winit `Window` is not `Clone`, has no `set_focus`, and `request_close` does not exist in 0.29.15.)
- Help: About → `Dialog::About`.

### Input handling (`handle_editor_input`)

- Mouse: `ctx.input(|i| i.pointer.interact_pos())`; `button_pressed/button_released/button_down(PointerButton::Primary)`;
  `button_double_clicked(PointerButton::Primary)` → `editor::word_range`; **there is no plain `double_clicked()`**.
  Press inside editor rect → hit-test char, `sel.collapse(ch)`, `dragging = true`; drag → `sel.head = hit`; release → `dragging = false`.
- Hit test: `y = p.y - rect.min.y - PAD + scroll.offset`, `vl = y / row_h`, `x = p.x - rect.min.x - PAD + scroll_x`, `col = x / glyph_w` → `editor::char_at_visual`.
  `PAD = 8.0`.
- Wheel: `let d = ctx.input(|i| i.smooth_scroll_delta);` → `self.scroll.nudge(-d.y * 3.0)`; horizontal when `!wrap`: `scroll_x -= d.x * 3.0`. Animation continues via `step_scroll` at the end of `redraw`.
- Keyboard: collect `ctx.input(|i| i.events.iter().cloned().collect::<Vec<_>>())`.
  - `Event::Text(s)` → insert (replaces selection).
  - `Event::Key { key, pressed, modifiers, .. }`, only `pressed`:
    - Ctrl (or `modifiers.command`): N/O/S/S→SaveAs/W/Z/Z+Shift/F/G/G+Shift/H/A/X/C/V.
    - Plain: Enter/`NumpadEnter` → `\n`; Backspace/Delete; Tab → `\t`; arrows (Shift extends selection); Home/End; PageUp/PageDown; Escape → clear selection.
- Clipboard: `clipboard.get_text()` / `clipboard.set_text(...)`; normalize CRLF/CR on paste.
- Helper methods: `char_at(rect, pos, row_h, glyph_w)`, `char_at` uses `editor::char_at_visual`;
  `edit_insert(text, view_h, row_h, glyph_w)` (replaces selection, collapses caret after insert, scrolls caret into view);
  `move_visual(dir, shift, ...)`; `page_scroll(dir, ...)`; `scroll_to_char(ch, view_h, row_h, glyph_w)`;
  `step_scroll(view_h, row_h) -> bool` — velocity step with boundary stop (zero velocity at 0/max, else `*0.80`, return `true` while moving);
  `ensure_map(wrap, rect_w, glyph_w, font_size)` — recompute when `MapKey { edits, wrap, cols, font: (size*2) as u32 }` changes, `cols = ((rect_w - 16.0) / glyph_w) as usize`;
  `do_undo` / `do_redo` (restore caret from returned `Option<usize>`);
  `selection_text`, `do_copy`, `do_cut`, `do_paste`.
- Find/replace: `open_find()` copies `self.find` into the dialog. Actions:
  - `FindNext(forward)`: `from = if forward { sel.range().1 } else { sel.range().0 }`; on match `sel.set(s,e)`, `find_target = Some((s,e))`, scroll to `s`; else set the dialog error.
  - `Replace`: replace `find_target` (or find the next first), then advance to the next match.
  - `ReplaceAll`: `doc.replace_all(&matches, &replacement)`; report count in the dialog error label and in `self.note`.
  - Sync `Search` back into `self.find` after each action (match `self.dialog.as_mut()`).
- Modal keys: while modal only Escape is handled (close prompt → dismiss; dialog → `dialog = None`). Editor input is skipped entirely while modal so the dialogs' `TextEdit`s own the keyboard.

### Drawing, status bar, close prompt

- `draw_editor(ui, theme, &font_id, row_h, glyph_w, rect, wrap, font_size)`:
  clip to `rect`; `ensure_map`; compute visible visual lines from `scroll.offset`;
  current-line highlight (`theme.current_line`), selection rects (`theme.selection`), text via
  `painter.text(pos, Align2::LEFT_CENTER, chunk, font_id.clone(), theme.text_fg)`; when
  `selection_fg != text_fg` the selected run is repainted with `painter.with_clip_rect(sel_rect)`
  in `theme.selection_fg` (the Night inverse; glyph positions are identical because the whole
  line is drawn twice at the same x). Gutters use `theme.gutter_fg`, solid caret
  (1.5 px `theme.accent` bar at the caret cell — **no blink**: egui-multiwin installs no repaint callback and delayed redraws aren't possible).
- `status_bar`: `●` + name; `Ln l, Col c` (1-based); `{} bytes · {} words`; EOL label; encoding label; `note`; right-aligned wrap indicator.
  Drain `words_rx` each frame (`try_recv`, keep result only if `gen == doc.edits`); call `count_words()` lazily.
- `count_words()`: if `doc.edits != words_gen`, clone `doc.rope` (`Rope: Clone + Send`, documented thread-safe), spawn a thread counting words, send `(gen, count)` over `words_tx`. Keeps the UI responsive on 10 MB+ files.
- `close_dialog(&self, ctx, theme) -> CloseAct`: foreground `Area`, title "Unsaved Changes", message `"{} has unsaved changes. Save them before closing?"`, buttons Cancel / Don't Save / Save.
- `do_close_act`: Cancel → clear flag; Discard → `force_close = true; rr.quit = true`; Save → save now (or Save-As with `save_then_close = true`); on success `force_close = true; rr.quit = true`.
- `apply_action(act, c, rr, view_h, row_h, glyph_w)`:
  - `Close` → `dialog = None`
  - `OpenFile(path)` → `open_path(path, c, rr)`
  - `SaveFile(path)` → `fileio::save`; on success set `doc.path`, `modified = false`, recents, and `rr.quit = true` if `save_then_close`
  - `FindNext` / `Replace` / `ReplaceAll` as above

**How "close" works (verified in egui-multiwin 0.5.2):**
`CloseRequested` → `control_flow = None` → `try_quit` → `can_quit`. If it returns `false`, the window stays alive (and the next input event triggers a redraw, which shows the prompt). Returning `RedrawResponse { quit: true, .. }` from `redraw` starts the same flow, so a "programmatic close" is just `rr.quit = true` and goes through `can_quit`. `can_quit` is called **twice** (in `try_quit` and again in `do_window_events`) — keep it idempotent. When the last window is removed, `run()` calls `event_loop.exit()`.

---

## `src/main.rs` — paste-ready

```rust
//! Kanpeki — a fast, multi-window text editor in the spirit of Craig
//! Marciniak's SaintEdit (1993), the classic System 7 editor.

mod buffer;
mod dialogs;
mod editor;
mod fileio;
mod search;
mod theme;
mod windows;

use egui_multiwin::enum_dispatch::enum_dispatch;
use egui_multiwin::winit::window::WindowId;

use crate::egui_multiwin_dynamic::multi_window::NewWindowRequest;
use crate::egui_multiwin_dynamic::tracked_window::TrackedWindow;

/// Macro-generated egui-multiwin plumbing. Must be `pub`; types are `crate::`-qualified.
pub mod egui_multiwin_dynamic {
    egui_multiwin::tracked_window!(crate::AppCommon, crate::CustomEvent, crate::Windows);
    egui_multiwin::multi_window!(crate::AppCommon, crate::CustomEvent, crate::Windows);
}

/// Data shared by every window.
pub struct AppCommon {
    /// Persisted user settings.
    pub settings: theme::Settings,
    /// Open documents for the Window menu: (window id, title, dirty).
    pub doc_entries: Vec<(WindowId, String, bool)>,
    /// Most recently focused window (for the Window menu marker).
    pub active_window: Option<WindowId>,
    /// Cascade counter for new windows.
    pub window_seq: u32,
}

impl AppCommon {
    /// Add or refresh this window's entry in the document list.
    pub fn upsert_entry(&mut self, id: WindowId, title: &str, dirty: bool) {
        if let Some(e) = self.doc_entries.iter_mut().find(|e| e.0 == id) {
            e.1 = title.to_string();
            e.2 = dirty;
        } else {
            self.doc_entries.push((id, title.to_string(), dirty));
        }
    }

    /// Drop a window from the document list.
    pub fn remove_entry(&mut self, id: WindowId) {
        self.doc_entries.retain(|e| e.0 != id);
        if self.active_window == Some(id) {
            self.active_window = None;
        }
    }

    /// Called by the run loop for user events that have no window id.
    pub fn process_event(&mut self, _event: CustomEvent) -> Vec<NewWindowRequest> {
        Vec::new()
    }
}

/// Custom event type for the event loop.
#[derive(Debug, Clone)]
pub struct CustomEvent {
    window: Option<WindowId>,
}

impl CustomEvent {
    /// The target window, if any.
    pub fn window_id(&self) -> Option<WindowId> {
        self.window
    }
}

/// Every kind of window in the program.
#[enum_dispatch(TrackedWindow)]
pub enum Windows {
    /// A document window.
    Doc(windows::DocWindow),
}

fn main() {
    let mut common = AppCommon {
        settings: theme::Settings::load(),
        doc_entries: Vec::new(),
        active_window: None,
        window_seq: 0,
    };

    egui_multiwin_dynamic::multi_window::MultiWindow::start(|mw, event_loop, _proxy| {
        let req = windows::DocWindow::request(buffer::Doc::blank(), &mut common);
        if let Err(e) = mw.add(req, &mut common, event_loop) {
            eprintln!("Couldn't open the first window: {e:?}");
        }
        common
    })
    .expect("failed to run the event loop");
}
```

---

## Verified external API facts (do not re-research)

**egui-multiwin 0.5.2** (`~/.cargo/registry/src/index.crates.io-*/egui-multiwin-0.5.2/src/`)

- `TrackedWindow` trait (generated in `tracked_window` module):
  - `fn is_root(&self) -> bool` (default false)
  - `fn can_quit(&mut self, c: &mut Common) -> bool` (default true)
  - `fn custom_event(&mut self, &Event, &mut Common, &mut EguiGlow, &winit::window::Window, &mut arboard::Clipboard) -> RedrawResponse` (default)
  - `fn redraw(&mut self, &mut Common, &mut EguiGlow, &winit::window::Window, &mut arboard::Clipboard) -> RedrawResponse`
  - `unsafe fn opengl_before/after(...)` (default no-op)
- `RedrawResponse { pub quit: bool, pub new_windows: Vec<NewWindowRequest> }` + `Default`.
- `NewWindowRequest::new(window_state: Windows, WindowBuilder, TrackedWindowOptions, id: u32)`; id from `egui_multiwin::multi_window::new_id()`.
- `TrackedWindowOptions { vsync: bool, shader: Option<egui_glow::ShaderVersion> }`.
- `MultiWindow::start(|&mut MultiWindow, &EventLoop, EventLoopProxy| -> Common) -> Result<(), EventLoopError>`; `add(req, &mut common, event_loop)`.
- The common struct must have an **inherent** `process_event(&mut self, Event) -> Vec<NewWindowRequest>`; the event type must have an inherent `window_id(&self) -> Option<WindowId>`.
- Routing: `Event::UserEvent` with `window_id() == Some(id)` goes only to that window (`is_event_for_window`); `None` goes to `process_event`.
- Each window has its own `EguiGlow`, and the crate calls `begin_frame` **before** `redraw` and `end_frame` **after** — `redraw` acts as a normal egui frame.
- No repaint callback is installed → `ctx.request_repaint()` is a no-op. Use `window.request_redraw()`.
- Exit: when the window list becomes empty, `run()` calls `event_loop.exit()`.

**winit 0.29.15** — `Window`: `id()`, `has_focus()`, `set_title(&str)`, `request_redraw()`, `set_visible(bool)`, `focus_window()`. **No** `close()`, `request_close()`, or `set_focus()`. `Window` is **not** `Clone`.

**egui 0.27.2**

- Input: `InputState { raw, pointer, raw_scroll_delta, smooth_scroll_delta, modifiers, events, focused }`.
- `Event`: `Text(String)`, `Key { key, pressed, repeat, modifiers }`, `PointerMoved(Pos2)`, `PointerButton { pos, button, pressed, modifiers }`, `Scroll(Vec2)`.
- `PointerState`: `interact_pos()`, `latest_pos()`, `button_pressed(PointerButton)`, `button_released(...)`, `button_down(...)`, `primary_clicked()`, `button_double_clicked(...)`, `button_triple_clicked(...)`. **No plain `double_clicked()`.**
- `Modifiers { alt, ctrl, shift, mac_cmd, command }` — use `ctrl || command`.
- `Context::fonts(|f| ...)`, `Fonts::row_height(&FontId)`, `Fonts::glyph_width(&FontId, char)`.
- `Painter::text(pos, Align2, text, FontId, Color32)`, `rect_filled(rect, rounding, color)`, `rect_stroke(rect, rounding, stroke)` — **no `StrokeKind` in 0.27**; `egui::Frame::none()` — **no `Frame::NONE` in 0.27**. `Frame` builders: `.fill()`, `.stroke()`, `.rounding()`, `.inner_margin(Margin::same(f32))`.
- `Area::new(Id).fixed_pos(pos).order(Order::Foreground).show(ctx, ..)`.
- `ui.menu_button(title, |ui| ...)` has no `open` argument; `ui.close_menu()` closes it.
- `ui.add_enabled(bool, Button::new(...))` for disabled menu items.
- Default fonts: proportional "Ubuntu-Light", monospace "Hack". `FontFamily::Monospace` is fine as-is. (Noto Sans Mono / Liberation Mono exist on the machine if a nicer face is wanted — `MultiWindow::add_font` registers a *named* family, so you'd use `FontFamily::Name("mono".into())` in the editor `FontId`.)

**ropey 1.6.1** — `Rope` is `Clone` and documented thread-safe (clone is cheap, Arc-based). `get_line(idx) -> Option<RopeSlice>`, `insert(char_idx, &str)`, `remove(Range<usize>)`, `slice(..)`, `chars()`, `len_chars/len_bytes/len_lines`, `char_to_line`, `line_to_char`.

---

## Remaining work checklist

Everything required is implemented, building, and exercised on the X11 desktop.
All items from the old optional-polish list are now implemented; see "Polish
added after 1.0.0" above.

### Acceptance tests

- Open/close multiple windows; app exits when the last window closes.
- Ctrl+N/O/S/W, Ctrl+Z / Ctrl+Shift+Z, Ctrl+X/C/V, Ctrl+A, Ctrl+F/G/H all work.
- Unsaved-changes prompt appears on WM close and Ctrl+W; Save / Don't Save / Cancel all behave.
- 10 MB file: opens fast, scrolls smoothly (inertia), typing stays responsive (word count is off-thread).
- CRLF file round-trips byte-identically on save; classic-Mac (CR) file loads and saves as CR; UTF-8 BOM preserved; UTF-16 loads as UTF-8.
- Find/replace: plain, regex, case-sensitive, whole-word, Replace All as one undo step.
- Word wrap toggle; font A−/A+; Day/Night persist across restarts.
- Window menu lists documents; title bar shows `●` when modified.
- The shiba inu window icon is embedded as a 128×128 PNG. 256×256 **fails**: the `_NET_WM_ICON` property would exceed X11's default maximum request size (262 140 bytes) and winit silently leaves the property empty, so `xprop` shows no icon. 128×128 is (128² + 2) × 4 ≈ 65 KB and works.

## License

Released into the public domain under the Unlicense; see `LICENSE`.
