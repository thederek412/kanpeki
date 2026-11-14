//! Editor geometry: visual lines (word wrap), smooth FastScrolls-style
//! scrolling, hit-testing, and selection helpers.

use crate::buffer::Doc;

/// One visual (screen) line: a slice of one document line.
#[derive(Clone, Copy, Debug)]
pub struct VisualLine {
    /// Char index in the document where this visual line starts.
    pub char_start: usize,
    /// The document line number this visual line belongs to.
    pub row: usize,
    /// How many characters are in this visual line.
    pub len: usize,
}

/// Maps document characters to visual lines (one per row when wrap is off).
pub struct VisualMap {
    /// The visual lines, in order.
    pub lines: Vec<VisualLine>,
    /// Widest visual line, in monospace cells (0 while word wrap is on,
    /// which never scrolls horizontally).
    pub max_width: usize,
}

/// Display width of a string in monospace cells (a tab is 4 cells).
pub fn visual_width(s: &str) -> usize {
    s.chars().map(|c| if c == '\t' { 4 } else { 1 }).count()
}

/// Build the visual-line table for the whole document.
///
/// O(document size) — cached by the window and rebuilt only when the text,
/// the wrap setting, or the column count changes.
pub fn build_visual_map(doc: &Doc, wrap: bool, cols: usize) -> VisualMap {
    let nlines = doc.len_lines();
    let mut lines: Vec<VisualLine> = Vec::with_capacity(nlines);
    let mut max_width = 0usize;
    for row in 0..nlines {
        let start = doc.line_start(row);
        let text = doc.line_text(row);
        if !wrap || cols == 0 {
            let w = visual_width(&text);
            max_width = max_width.max(w);
            lines.push(VisualLine {
                char_start: start,
                row,
                len: text.chars().count(),
            });
            continue;
        }
        // Split the line into chunks of `cols` visual cells (tab = 4).
        let mut chunks: Vec<(usize, usize)> = Vec::new(); // (char offset, char len)
        let mut off = 0usize;
        let mut v = 0usize;
        let mut chunk_off = 0usize;
        for ch in text.chars() {
            let w = if ch == '\t' { 4 } else { 1 };
            if v > 0 && v + w > cols {
                chunks.push((chunk_off, off - chunk_off));
                chunk_off = off;
                v = 0;
            }
            v += w;
            off += 1;
        }
        if off > chunk_off || chunks.is_empty() {
            chunks.push((chunk_off, off - chunk_off));
        }
        for (co, cl) in chunks {
            lines.push(VisualLine { char_start: start + co, row, len: cl });
        }
    }
    VisualMap { lines, max_width }
}

/// The visual line index containing character `idx`.
pub fn visual_of_char(map: &VisualMap, idx: usize) -> usize {
    if map.lines.is_empty() {
        return 0;
    }
    let mut lo = 0usize;
    let mut hi = map.lines.len();
    while lo < hi {
        let mid = (lo + hi) / 2;
        if map.lines[mid].char_start <= idx {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo.saturating_sub(1)
}

/// The character index at visual line `vl`, visual column `col` (relative to the visual line).
pub fn char_at_visual(doc: &Doc, map: &VisualMap, vl: usize, col: usize) -> usize {
    if map.lines.is_empty() {
        return 0;
    }
    let vl = vl.min(map.lines.len() - 1);
    let line = &map.lines[vl];
    let text = doc.line_text(line.row);
    let co = line.char_start.saturating_sub(doc.line_start(line.row));
    let mut v = 0usize;
    for (i, ch) in text.char_indices().skip(co) {
        if v >= col {
            return line.char_start + (i - co);
        }
        v += if ch == '\t' { 4 } else { 1 };
    }
    (line.char_start + line.len).min(doc.len_chars())
}

/// The visual column of character `char_idx` within visual line `vl`.
pub fn visual_col_of(doc: &Doc, map: &VisualMap, vl: usize, char_idx: usize) -> usize {
    if map.lines.is_empty() {
        return 0;
    }
    let vl = vl.min(map.lines.len() - 1);
    let line = &map.lines[vl];
    let text = doc.line_text(line.row);
    let co = line.char_start.saturating_sub(doc.line_start(line.row));
    let mut v = 0usize;
    for (i, ch) in text.char_indices().skip(co) {
        if line.char_start + (i - co) >= char_idx {
            return v;
        }
        v += if ch == '\t' { 4 } else { 1 };
    }
    v
}

/// Substring of `s` from char index `a` (inclusive) to `b` (exclusive), char-safe.
pub fn char_slice(s: &str, a: usize, b: usize) -> &str {
    let n = s.chars().count();
    let a = a.min(n);
    let b = b.min(n).max(a);
    let ba = s.char_indices().nth(a).map(|(i, _)| i).unwrap_or(s.len());
    let bb = s.char_indices().nth(b).map(|(i, _)| i).unwrap_or(s.len());
    &s[ba.min(bb)..bb]
}

/// The (character) range of the word containing `idx`.
pub fn word_range(doc: &Doc, idx: usize) -> (usize, usize) {
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let n = doc.len_chars();
    if n == 0 {
        return (0, 0);
    }
    let probe = idx.min(n - 1);
    let start_char = doc.get_char(probe).unwrap_or(' ');
    let anchor = if is_word(start_char) {
        probe
    } else if probe + 1 < n && is_word(doc.get_char(probe + 1).unwrap_or(' ')) {
        probe + 1
    } else {
        return (probe, probe + 1);
    };
    let mut s = anchor;
    while s > 0 {
        match doc.get_char(s - 1) {
            Some(c) if is_word(c) => s -= 1,
            _ => break,
        }
    }
    let mut e = anchor;
    while e < n {
        match doc.get_char(e) {
            Some(c) if is_word(c) => e += 1,
            _ => break,
        }
    }
    (s, e)
}

/// Smooth (inertial) scrolling, FastScrolls-style: the wheel adds velocity,
/// and the offset glides to rest each frame.
pub struct Scroll {
    /// Scroll offset in pixels from the top.
    pub offset: f32,
    /// Pixels added per frame; decays toward zero.
    pub velocity: f32,
}

impl Scroll {
    pub fn new() -> Self {
        Self { offset: 0.0, velocity: 0.0 }
    }

    /// Add to the scroll velocity (from a mouse wheel).
    pub fn nudge(&mut self, delta: f32) {
        self.velocity += delta;
    }

    /// Advance the animation by one frame. Returns true while still moving.
    pub fn step(&mut self) -> bool {
        if self.velocity.abs() > 0.02 {
            self.offset += self.velocity;
            self.velocity *= 0.80;
            true
        } else {
            self.velocity = 0.0;
            false
        }
    }

    /// Keep the offset inside `[0, max]`.
    pub fn clamp(&mut self, max: f32) {
        self.offset = self.offset.clamp(0.0, max.max(0.0));
    }
}

impl Default for Scroll {
    fn default() -> Self {
        Self::new()
    }
}

/// Anchor + head, in character indices.
#[derive(Clone, Copy, Default)]
pub struct Selection {
    /// The end that stays put while the selection grows.
    pub anchor: usize,
    /// The moving end (also the caret).
    pub head: usize,
}

impl Selection {
    /// The inclusive-exclusive range, in order.
    pub fn range(&self) -> (usize, usize) {
        (self.anchor.min(self.head), self.anchor.max(self.head))
    }

    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    pub fn set(&mut self, anchor: usize, head: usize) {
        self.anchor = anchor;
        self.head = head;
    }

    /// Collapse to a single caret position.
    pub fn collapse(&mut self, pos: usize) {
        self.anchor = pos;
        self.head = pos;
    }
}
