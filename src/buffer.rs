//! The text document: a ropey buffer with an undo/redo stack and file metadata.
//!
//! Text is always stored with Unix newlines (`\n`). Line endings are
//! detected on load and applied again on save, in the spirit of
//! SaintEdit's EOL conversion.

use ropey::Rope;

/// Line-ending style, as in the original SaintEdit (Unix / DOS / classic Mac).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Eol {
    /// `\n`
    #[default]
    Unix,
    /// `\r\n`
    Dos,
    /// `\r` — the classic Mac style SaintEdit itself was written for.
    Classic,
}

impl Eol {
    pub fn bytes(self) -> &'static [u8] {
        match self {
            Eol::Unix => b"\n",
            Eol::Dos => b"\r\n",
            Eol::Classic => b"\r",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Eol::Unix => "LF",
            Eol::Dos => "CRLF",
            Eol::Classic => "CR",
        }
    }
}

/// How the file was encoded when it was loaded.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Encoding {
    #[default]
    Utf8,
    Utf8Bom,
    /// UTF-16 (either byte order). Saved back out as UTF-8.
    Utf16,
    /// 8-bit, non-UTF-8. Saved back out as UTF-8.
    Latin1,
}

impl Encoding {
    pub fn label(self) -> &'static str {
        match self {
            Encoding::Utf8 => "UTF-8",
            Encoding::Utf8Bom => "UTF-8 (BOM)",
            Encoding::Utf16 => "UTF-16 → UTF-8",
            Encoding::Latin1 => "Latin-1 → UTF-8",
        }
    }
}

/// A single undoable edit.
enum Op {
    /// Insert `text` at `at`.
    Insert { at: usize, text: String },
    /// Delete the text at `at` (kept in `text` for redo).
    Delete { at: usize, text: String },
    /// Replace the range at `at` (length `old`) with `new`.
    Replace { at: usize, old: String, new: String },
    /// Many disjoint replacements, applied as one step (replace-all).
    ReplaceAll { segs: Vec<(usize, String, String)> },
}

const UNDO_CAP: usize = 400;

/// A text document: the rope, where it lives on disk, and its undo stacks.
pub struct Doc {
    /// The text itself (newlines normalized to `\n`).
    pub rope: Rope,
    /// The file this document is associated with, if any.
    pub path: Option<std::path::PathBuf>,
    /// True when the document differs from what is on disk.
    pub modified: bool,
    /// The line-ending style to use when saving.
    pub eol: Eol,
    /// How the document was originally encoded.
    pub encoding: Encoding,
    /// Cached word count; `None` means "recompute".
    pub word_cache: Option<usize>,
    /// Bumped on every text change; used to invalidate cached views (e.g. the wrap table).
    pub edits: u64,
    undo: Vec<Op>,
    redo: Vec<Op>,
}

impl Doc {
    /// An empty document.
    pub fn blank() -> Self {
        Self {
            rope: Rope::new(),
            path: None,
            modified: false,
            eol: Eol::Unix,
            encoding: Encoding::Utf8,
            word_cache: None,
            edits: 0,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    /// Create a document from text, normalizing line endings to `\n`.
    pub fn from_text(
        text: &str,
        eol: Eol,
        encoding: Encoding,
        path: Option<std::path::PathBuf>,
    ) -> Self {
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        Self {
            rope: Rope::from_str(&text),
            path,
            modified: false,
            eol,
            encoding,
            word_cache: None,
            edits: 0,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    pub fn len_bytes(&self) -> usize {
        self.rope.len_bytes()
    }

    pub fn len_lines(&self) -> usize {
        self.rope.len_lines()
    }

    pub fn get_char(&self, idx: usize) -> Option<char> {
        self.rope.get_char(idx)
    }

    pub fn char_to_line(&self, idx: usize) -> usize {
        self.rope.char_to_line(idx.min(self.len_chars()))
    }

    pub fn line_start(&self, line: usize) -> usize {
        if line >= self.len_lines() {
            return self.len_chars();
        }
        self.rope.line_to_char(line)
    }

    pub fn line_text(&self, line: usize) -> String {
        // ropey's `get_line` includes the line's trailing '\n'; the editor
        // wants the visible text only.
        let raw: String = self.rope.get_line(line).map(|s| s.into()).unwrap_or_default();
        match raw.strip_suffix('\n') {
            Some(t) => t.to_string(),
            None => raw,
        }
    }

    /// The full text as a `String` (for search, save, copy).
    pub fn to_string(&self) -> String {
        self.rope.chars().collect()
    }

    fn push_undo(&mut self, op: Op) {
        self.undo.push(op);
        if self.undo.len() > UNDO_CAP {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.modified = true;
        self.word_cache = None;
        self.edits += 1;
    }

    /// Insert text at `at` (undoable).
    pub fn insert(&mut self, at: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        let at = at.min(self.len_chars());
        self.push_undo(Op::Insert { at, text: text.to_string() });
        self.rope.insert(at, text);
    }

    /// Delete the range `[start, end)` (undoable).
    pub fn delete(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let end = end.min(self.len_chars());
        let text: String = self.rope.slice(start..end).into();
        self.push_undo(Op::Delete { at: start, text });
        self.rope.remove(start..end);
    }

    /// Replace the range `[start, end)` with `text` as a single undo step.
    pub fn replace_range(&mut self, start: usize, end: usize, text: &str) {
        if start >= end {
            self.insert(start, text);
            return;
        }
        let end = end.min(self.len_chars());
        let old: String = self.rope.slice(start..end).into();
        if old == text {
            return;
        }
        self.push_undo(Op::Replace { at: start, old, new: text.to_string() });
        self.rope.remove(start..end);
        self.rope.insert(start, text);
    }

    /// Replace every range in `matches` with `replacement` as a single undo step.
    /// Returns how many replacements were made.
    pub fn replace_all(&mut self, matches: &[(usize, usize)], replacement: &str) -> usize {
        if matches.is_empty() {
            return 0;
        }
        let segs: Vec<(usize, String, String)> = matches
            .iter()
            .map(|&(s, e)| (s, self.rope.slice(s..e).into(), replacement.to_string()))
            .collect();
        self.push_undo(Op::ReplaceAll { segs: segs.clone() });
        for &(s, e) in matches.iter().rev() {
            self.rope.remove(s..e);
            self.rope.insert(s, replacement);
        }
        matches.len()
    }

    fn apply_undo_op(&mut self, op: &Op) {
        match op {
            Op::Insert { at, text } => {
                self.rope.remove(*at..*at + text.chars().count());
            }
            Op::Delete { at, text } => {
                self.rope.insert(*at, text);
            }
            Op::Replace { at, old, new } => {
                self.rope.remove(*at..*at + new.chars().count());
                self.rope.insert(*at, old);
            }
            Op::ReplaceAll { segs } => {
                for &(s, ref old, ref new) in segs.iter().rev() {
                    self.rope.remove(s..s + new.chars().count());
                    self.rope.insert(s, old);
                }
            }
        }
    }

    fn apply_redo_op(&mut self, op: &Op) {
        match op {
            Op::Insert { at, text } => {
                self.rope.insert(*at, text);
            }
            Op::Delete { at, text } => {
                self.rope.remove(*at..*at + text.chars().count());
            }
            Op::Replace { at, old, new } => {
                self.rope.remove(*at..*at + old.chars().count());
                self.rope.insert(*at, new);
            }
            Op::ReplaceAll { segs } => {
                for &(s, ref old, ref new) in segs.iter().rev() {
                    self.rope.remove(s..s + old.chars().count());
                    self.rope.insert(s, new);
                }
            }
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Undo the last edit. Returns the caret position to restore, if anything was undone.
    pub fn undo(&mut self) -> Option<usize> {
        let op = self.undo.pop()?;
        let caret = match &op {
            Op::Insert { at, .. } => *at,
            Op::Delete { at, .. } => *at,
            Op::Replace { at, .. } => *at,
            Op::ReplaceAll { segs } => segs.first().map(|(s, _, _)| *s).unwrap_or(0),
        };
        self.apply_undo_op(&op);
        self.redo.push(op);
        self.modified = true;
        self.word_cache = None;
        self.edits += 1;
        Some(caret)
    }

    /// Redo the last undone edit. Returns the caret position to restore, if anything was redone.
    pub fn redo(&mut self) -> Option<usize> {
        let op = self.redo.pop()?;
        let caret = match &op {
            Op::Insert { at, text } => *at + text.chars().count(),
            Op::Delete { at, .. } => *at,
            Op::Replace { at, new, .. } => *at + new.chars().count(),
            Op::ReplaceAll { segs } => segs.last().map(|(s, _, n)| *s + n.chars().count()).unwrap_or(0),
        };
        self.apply_redo_op(&op);
        self.undo.push(op);
        self.modified = true;
        self.word_cache = None;
        self.edits += 1;
        Some(caret)
    }

    /// Word count (cached; recomputed lazily after edits).
    pub fn word_count(&mut self) -> usize {
        if self.word_cache.is_none() {
            let mut words = 0usize;
            let mut in_word = false;
            for ch in self.rope.chars() {
                if ch.is_whitespace() {
                    in_word = false;
                } else if !in_word {
                    in_word = true;
                    words += 1;
                }
            }
            self.word_cache = Some(words);
        }
        self.word_cache.unwrap_or(0)
    }
}

impl Default for Doc {
    fn default() -> Self {
        Self::blank()
    }
}
