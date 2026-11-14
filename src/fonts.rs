//! The bundled editor typeface: Hurmit Nerd Font Mono.
//!
//! The font travels inside the binary, so using Kanpeki never requires
//! installing a font. It is distributed under the SIL Open Font License;
//! see `assets/fonts/HurmitNerdFont-LICENSE.txt`.

use egui_multiwin::egui::FontData;

/// Egui family name under which the font is registered.
pub const FAMILY: &str = "kanpeki-mono";

/// A short label for the status bar.
pub const LABEL: &str = "Hurmit Nerd Font";

/// The bundled font (regular weight, strictly monospaced Nerd Font build).
pub fn bundled() -> FontData {
    FontData::from_static(include_bytes!(
        "../assets/fonts/HurmitNerdFontMono-Regular.otf"
    ))
}
