//! Escaping of user values inside FFmpeg filter graphs.

use crate::*;

pub(crate) fn ffmpeg_filter_path(path: &Path) -> String {
    ffmpeg_quoted_value(&path.to_string_lossy().replace('\\', "/"))
}

pub(crate) fn ffmpeg_quoted_value(value: &str) -> String {
    // Callers wrap this in single quotes. Escape the option parser first, then
    // close/reopen graph-level quotes around apostrophes. One backslash alone
    // loses the apostrophe when FFmpeg parses the filter graph a second time.
    let mut option = String::with_capacity(value.len());
    for c in value.chars() {
        if matches!(c, '\\' | ':' | '\'') || c.is_ascii_whitespace() {
            option.push('\\');
        }
        option.push(c);
    }
    option.replace('\'', "'\\''")
}

pub(crate) fn drawtext_escape(value: &str) -> String {
    // drawtext callers also set expansion=none, so percent sequences are text,
    // never expressions. This is shared by Watermark, Social Tag and text layers.
    ffmpeg_quoted_value(value)
}

pub(crate) fn movie_filter_path(path: &Path) -> String {
    // Escape both the filter-option parser and the surrounding graph parser.
    // Forward slashes avoid treating Windows separators as escape characters.
    let option = path
        .to_string_lossy()
        .replace('\\', "/")
        .replace(':', "\\:")
        .replace('\'', "\\'");
    option
        .chars()
        .flat_map(|c| {
            if matches!(c, '\\' | '\'' | '[' | ']' | ',' | ';') {
                vec!['\\', c]
            } else {
                vec![c]
            }
        })
        .collect()
}
