use pyo3::prelude::*;
use tslib_chat::bbcode::{strip_bbcode, BBCodeParser};

/// Convert BBCode markup to HTML.
#[pyfunction]
pub fn bbcode_to_html(input: String) -> String {
    BBCodeParser::to_html(&input)
}

/// Convert BBCode markup to plain text.
#[pyfunction]
pub fn bbcode_to_plain(input: String) -> String {
    BBCodeParser::to_plain(&input)
}

/// Convert BBCode markup to ANSI terminal sequences.
#[pyfunction]
pub fn bbcode_to_ansi(input: String) -> String {
    BBCodeParser::to_ansi(&input)
}

/// Strip all BBCode tags from text, returning plain text.
#[pyfunction]
pub fn py_strip_bbcode(input: String) -> String {
    strip_bbcode(&input)
}
