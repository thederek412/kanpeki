//! The application icon: a shiba inu, embedded in the binary.

use std::sync::OnceLock;

use egui_multiwin::winit::window::Icon;

/// The icon is a 128x128 RGBA PNG kept in `assets/`.
const ICON_PNG: &[u8] = include_bytes!("../assets/kanpeki-icon.png");

/// The window/taskbar icon, decoded once and cloned for each new window.
pub fn app_icon() -> Option<Icon> {
    static ICON: OnceLock<Option<Icon>> = OnceLock::new();
    ICON.get_or_init(|| decode(ICON_PNG)).clone()
}

/// Decode the embedded PNG into the RGBA buffer winit expects.
fn decode(bytes: &[u8]) -> Option<Icon> {
    let mut decoder = png::Decoder::new(bytes);
    // Normalise palette/grey/16-bit PNGs to 8-bit RGB or RGBA.
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    let data = &buf[..info.buffer_size()];

    let rgba = match info.color_type {
        png::ColorType::Rgba => data.to_vec(),
        png::ColorType::Rgb => {
            let mut out = Vec::with_capacity(data.len() / 3 * 4);
            for px in data.chunks_exact(3) {
                out.extend_from_slice(px);
                out.push(255);
            }
            out
        }
        png::ColorType::GrayscaleAlpha => {
            let mut out = Vec::with_capacity(data.len() / 2 * 4);
            for px in data.chunks_exact(2) {
                out.extend_from_slice(&[px[0], px[0], px[0], px[1]]);
            }
            out
        }
        png::ColorType::Grayscale => {
            let mut out = Vec::with_capacity(data.len() * 4);
            for &g in data {
                out.extend_from_slice(&[g, g, g, 255]);
            }
            out
        }
        _ => return None,
    };

    Icon::from_rgba(rgba, info.width, info.height).ok()
}
