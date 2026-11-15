//! Search & replace: plain text, regular expressions, case sensitivity,
//! whole words — in the spirit of SaintEdit's find & replace.

use crate::buffer::Doc;

/// The state of the find/replace dialog.
#[derive(Clone, Default)]
pub struct Search {
    /// What to look for.
    pub query: String,
    /// What to replace it with.
    pub replacement: String,
    /// Match case exactly.
    pub case_sensitive: bool,
    /// Interpret the query as a regular expression.
    pub regex: bool,
    /// Only match whole words.
    pub whole_word: bool,
}

impl Search {
    /// Compile the current settings into a regular expression.
    pub fn compile(&self) -> Result<regex::Regex, String> {
        let pat = if self.regex {
            let p = self.query.clone();
            if self.whole_word {
                format!(r"\b(?:{})\b", p)
            } else {
                p
            }
        } else {
            let p = regex::escape(&self.query);
            if self.whole_word {
                format!(r"\b(?:{})\b", p)
            } else {
                p
            }
        };
        let pat = if self.case_sensitive {
            pat
        } else {
            format!("(?i){pat}")
        };
        regex::Regex::new(&pat).map_err(|e| format!("Invalid pattern: {e}"))
    }

    /// All matches in the document, in order.
    ///
    /// The `regex` crate reports match positions as **byte** offsets, but
    /// `Doc` (and therefore the editor's selection, caret, and replace API)
    /// works in **character** indices. For files with non-ASCII text before a
    /// match — em dashes, accented letters, and so on — the two differ, so
    /// each match is translated before being returned.
    pub fn find_all(&self, doc: &Doc) -> Result<Vec<(usize, usize)>, String> {
        if self.query.is_empty() {
            return Ok(Vec::new());
        }
        let re = self.compile()?;
        let text = doc.to_string();
        let mut out = Vec::new();

        // Walk the matches in order and convert byte offsets to char indices
        // incrementally. Each byte of `text` is scanned at most once, so this
        // stays O(n) without allocating a byte→char table.
        let mut byte_pos = 0usize;
        let mut char_pos = 0usize;
        for m in re.find_iter(&text) {
            char_pos += text[byte_pos..m.start()].chars().count();
            let char_len = text[m.start()..m.end()].chars().count();
            out.push((char_pos, char_pos + char_len));
            char_pos += char_len;
            byte_pos = m.end();
        }
        Ok(out)
    }

    /// The next match at/after `from` (or the last one before `from`),
    /// wrapping around to the beginning/end.
    pub fn find_next(&self, doc: &Doc, from: usize, forward: bool) -> Result<Option<(usize, usize)>, String> {
        let all = self.find_all(doc)?;
        if all.is_empty() {
            return Ok(None);
        }
        if forward {
            for &(s, e) in &all {
                if e > from {
                    return Ok(Some((s, e)));
                }
            }
            Ok(Some(all[0]))
        } else {
            for &(s, e) in all.iter().rev() {
                if s < from {
                    return Ok(Some((s, e)));
                }
            }
            Ok(Some(*all.last().unwrap()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::{Doc, Encoding, Eol};

    #[test]
    fn find_all_returns_char_indices_not_byte_offsets() {
        // The em dash before the date is one character but three bytes, so
        // byte and character offsets diverge as soon as it is scanned.
        let mut doc = Doc::from_text(
            "Arugula  —  2018-01-01  (p.45)\nBread  —  2018-01-01  (p.41)\n",
            Eol::Unix,
            Encoding::Utf8,
            None,
        );
        let search = Search {
            query: "2018-01".to_string(),
            ..Search::default()
        };

        let matches = search.find_all(&doc).unwrap();
        assert_eq!(matches.len(), 2);

        for &(s, e) in &matches {
            let matched: String = doc.rope.slice(s..e).into();
            assert_eq!(matched, "2018-01", "match at ({s}, {e}) selected the wrong text");
        }

        // Search, selection, and replace all share the same indices, so
        // replace-all must also land on the actual text (this would remove
        // the wrong characters if byte offsets leaked through).
        doc.replace_all(&matches, "2099-12");
        assert!(doc.to_string().contains("Arugula  —  2099-12-01"));
        assert!(doc.to_string().contains("Bread  —  2099-12-01"));
    }
}
