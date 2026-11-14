# Kanpeki

A fast, multi-window text editor for the desktop, written in Rust in the spirit
of the classic System 7 editor **SaintEdit** (Craig Marciniak, 1993).

Named for the Japanese word for “perfect”.

## Features

- One true OS window per document, with a Window menu listing every open file
- Deep undo/redo and atomic, crash-safe saves (temp file + fsync + rename)
- Find & replace: plain text or regex, case-sensitive, whole-word
- Day (`#9399b7`) and Night (`#32364a`) palettes
- Smooth inertial scrolling, word wrap, optional line numbers
- Unix, DOS and classic-Mac line endings detected and preserved on save
- Reads UTF-8 (with BOM), UTF-16 and Latin-1; saves as UTF-8
- Large-file friendly (ropey), with word counting on a worker thread
- Native file dialogs for Open and Save (zenity), with built-in fallbacks

## Build

Requires a stable Rust toolchain.

```bash
cargo build --release
./target/release/kanpeki
```

Linux/X11 (built on egui + egui-multiwin + winit).

## Notes

`HANDOFF.md` contains the architecture, implementation notes, and the external
API details that shaped the build.

## Credits

Created by Derek Kalinosky.
Inspired by Craig Marciniak’s SaintEdit (1993) — thank you, Craig.
