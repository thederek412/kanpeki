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
    pub fn find_all(&self, doc: &Doc) -> Result<Vec<(usize, usize)>, String> {
        if self.query.is_empty() {
            return Ok(Vec::new());
        }
        let re = self.compile()?;
        let text = doc.to_string();
        let mut out = Vec::new();
        for m in re.find_iter(&text) {
            out.push((m.start(), m.end()));
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
