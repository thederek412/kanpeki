//! File loading and saving: encoding detection (UTF-8/UTF-16 BOM/Latin-1),
//! line-ending detection, and a safe (atomic) save: write to a temp file,
//! `fsync`, then rename over the original — the same "safe save" idea the
//! original SaintEdit had.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::buffer::{Doc, Eol, Encoding};

/// The result of reading a file.
pub struct Loaded {
    /// The decoded text (line endings NOT yet normalized).
    pub text: String,
    /// The dominant line-ending style in the file.
    pub eol: Eol,
    /// How the text was encoded.
    pub encoding: Encoding,
}

/// Read and decode a file.
pub fn load(path: &Path) -> Result<Loaded, String> {
    let bytes = fs::read(path).map_err(|e| format!("Couldn't read {}: {e}", path.display()))?;
    let (text, encoding) = decode(&bytes);
    Ok(Loaded {
        text,
        eol: detect_eol(&bytes),
        encoding,
    })
}

fn decode(bytes: &[u8]) -> (String, Encoding) {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return (String::from_utf8_lossy(&bytes[3..]).into_owned(), Encoding::Utf8Bom);
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return (decode_utf16(&bytes[2..], true), Encoding::Utf16);
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return (decode_utf16(&bytes[2..], false), Encoding::Utf16);
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => (s.to_string(), Encoding::Utf8),
        Err(_) => (bytes.iter().map(|b| *b as char).collect(), Encoding::Latin1),
    }
}

fn decode_utf16(bytes: &[u8], little: bool) -> String {
    let mut units = Vec::with_capacity(bytes.len() / 2);
    let mut i = 0;
    while i + 1 < bytes.len() {
        let pair = [bytes[i], bytes[i + 1]];
        units.push(if little {
            u16::from_le_bytes(pair)
        } else {
            u16::from_be_bytes(pair)
        });
        i += 2;
    }
    String::from_utf16_lossy(&units)
}

/// Detect the dominant line-ending style of the raw bytes.
fn detect_eol(bytes: &[u8]) -> Eol {
    let mut cr = 0usize;
    let mut lf = 0usize;
    let mut crlf = 0usize;
    for &b in bytes {
        if b == b'\n' {
            lf += 1;
        } else if b == b'\r' {
            cr += 1;
        }
    }
    if bytes.len() >= 2 {
        for w in bytes.windows(2) {
            if w[0] == b'\r' && w[1] == b'\n' {
                crlf += 1;
            }
        }
    }
    let lone_cr = cr - crlf;
    let lone_lf = lf - crlf;
    if crlf > lone_lf && crlf >= lone_cr {
        Eol::Dos
    } else if lone_cr > lone_lf {
        Eol::Classic
    } else {
        Eol::Unix
    }
}

/// Render the document for saving: apply the stored EOL style and BOM.
pub fn render_for_save(doc: &Doc) -> Vec<u8> {
    let text = doc.to_string();
    let final_text = match doc.eol {
        Eol::Unix => text,
        Eol::Dos => text.replace('\n', "\r\n"),
        Eol::Classic => text.replace('\n', "\r"),
    };
    let mut out = Vec::with_capacity(final_text.len() + 3);
    if doc.encoding == Encoding::Utf8Bom {
        out.extend_from_slice(b"\xEF\xBB\xBF");
    }
    out.extend_from_slice(final_text.as_bytes());
    out
}

/// Atomically save `doc` to `path`: temp file + fsync + rename.
pub fn save(doc: &Doc, path: &Path) -> Result<(), String> {
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    if !dir.is_dir() {
        return Err(format!("The folder {} doesn't exist", dir.display()));
    }
    let file = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("saintedit.tmp");
    let tmp = dir.join(format!(".saintedit-{}.tmp", file));

    let data = render_for_save(doc);
    let mut f = fs::File::create(&tmp).map_err(|e| format!("Couldn't create temp file: {e}"))?;
    f.write_all(&data).map_err(|e| format!("Write failed: {e}"))?;
    f.sync_all().map_err(|e| format!("Couldn't sync the file: {e}"))?;
    drop(f);

    if let Err(e) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(format!("Couldn't finish the save: {e}"));
    }
    // Best-effort: persist the directory entry too.
    if let Ok(dirf) = fs::File::open(dir) {
        let _ = dirf.sync_all();
    }
    Ok(())
}

/// The default directory to start an open/save dialog in.
pub fn default_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

/// List the entries of a directory, split into subfolders and files
/// (dotfiles hidden, both sorted).
pub fn dir_entries(dir: &Path) -> (Vec<String>, Vec<String>) {
    let mut dirs = Vec::new();
    let mut files = Vec::new();
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            if e.path().is_dir() {
                dirs.push(name);
            } else {
                files.push(name);
            }
        }
    }
    dirs.sort();
    files.sort();
    (dirs, files)
}
