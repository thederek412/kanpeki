//! Kanpeki — a fast, multi-window text editor in the spirit of Craig
//! Marciniak's SaintEdit (1993), the classic System 7 editor.

mod buffer;
mod dialogs;
mod editor;
mod fileio;
mod icon;
mod search;
mod theme;
mod windows;

use std::sync::Arc;

use egui_multiwin::egui_glow::EguiGlow;
use egui_multiwin::enum_dispatch::enum_dispatch;
use egui_multiwin::winit::window::WindowId;

use crate::egui_multiwin_dynamic::multi_window::NewWindowRequest;
use crate::egui_multiwin_dynamic::tracked_window::{RedrawResponse, TrackedWindow};

/// Macro-generated egui-multiwin plumbing. Must be `pub`; the types are
/// `crate::`-qualified because the macros expand inside this module.
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
    /// The most recently focused window (marked in the Window menu).
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
