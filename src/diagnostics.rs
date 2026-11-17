//! Diagnostics: severities, counters and the human-readable renderer.
//!
//! The engine never aborts and never exits: phases report what they find, the
//! driver inspects [`DiagnosticEngine::has_errors`] and decides the exit code.
//! Rendering is a pure function ([`render`]) so it can be unit-tested exactly.

use std::fmt::Write as _;
use std::io::{self, IsTerminal, Write};

use crate::source::{SourceManager, Span};

const COLOR_RESET: &str = "\x1b[0m";
const COLOR_RED: &str = "\x1b[0;1;31m";
const COLOR_YELLOW: &str = "\x1b[0;1;33m";
const COLOR_CYAN: &str = "\x1b[0;1;36m";
const COLOR_BOLD: &str = "\x1b[1m";

/// Column width used when expanding tabs in a source excerpt.
const TAB_WIDTH: usize = 4;

/// Severity of a diagnostic.
///
/// `Error` and `Fatal` make compilation fail; `Warning` and `Note` do not.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Severity {
    /// Extra context attached to a previous diagnostic.
    Note,
    /// Something suspicious that does not stop compilation.
    Warning,
    /// A violation of the language rules.
    Error,
    /// An error after which continuing would be meaningless.
    Fatal,
}

impl Severity {
    /// Short name used in the rendered diagnostic.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Severity::Note => "note",
            Severity::Warning => "warning",
            Severity::Error => "error",
            Severity::Fatal => "fatal error",
        }
    }

    const fn color(self) -> &'static str {
        match self {
            Severity::Note => COLOR_CYAN,
            Severity::Warning => COLOR_YELLOW,
            Severity::Error | Severity::Fatal => COLOR_RED,
        }
    }

    /// Returns whether this severity makes compilation fail.
    #[must_use]
    pub const fn is_error(self) -> bool {
        matches!(self, Severity::Error | Severity::Fatal)
    }
}

/// Renders one diagnostic to a string.
///
/// With a valid span the output is
///
/// ```text
/// sample.nova:2:5: error: unknown identifier 'y'
///   int y;
///       ^
/// ```
///
/// `color` only takes effect when the caller decides the sink supports ANSI
/// escapes.
#[must_use]
pub fn render(
    sources: &SourceManager,
    severity: Severity,
    span: Option<Span>,
    message: &str,
    color: bool,
) -> String {
    let mut out = String::new();

    // Location prefix and source excerpt, when a span points into a known file.
    let mut excerpt = None;
    if let Some(span) = span {
        if let Some(file) = sources.file(span.file) {
            let (line, column) = file.line_col(span.start);
            let _ = write!(out, "{}:{line}:{column}: ", file.path().display());
            let (line_text, _) = file.line_text(span.start);
            excerpt = Some(render_excerpt(line_text, column));
        }
    }

    if color {
        out.push_str(severity.color());
    }
    out.push_str(severity.label());
    out.push_str(": ");
    if color {
        out.push_str(COLOR_RESET);
    }
    out.push_str(message);
    out.push('\n');

    if let Some((rendered_line, caret_column)) = excerpt {
        out.push_str("  ");
        out.push_str(&rendered_line);
        out.push('\n');
        out.push_str("  ");
        out.extend(std::iter::repeat(' ').take(caret_column));
        if color {
            out.push_str(COLOR_BOLD);
        }
        out.push('^');
        if color {
            out.push_str(COLOR_RESET);
        }
        out.push('\n');
    }

    out
}

/// Renders the source line with tabs expanded and returns the caret position in
/// the rendered coordinates.
fn render_excerpt(line: &str, column: u32) -> (String, usize) {
    let target_byte_index = column.saturating_sub(1) as usize;
    let mut rendered = String::with_capacity(line.len());
    let mut visual = 0usize;
    let mut caret = None;

    for (byte_index, character) in line.char_indices() {
        if byte_index == target_byte_index {
            caret = Some(visual);
        }
        if character == '\t' {
            let spaces = TAB_WIDTH - (visual % TAB_WIDTH);
            rendered.extend(std::iter::repeat(' ').take(spaces));
            visual += spaces;
        } else {
            rendered.push(character);
            visual += 1;
        }
    }

    (rendered, caret.unwrap_or(visual))
}

/// Collects diagnostics, prints them and keeps counts.
#[derive(Debug)]
pub struct DiagnosticEngine {
    errors: u32,
    warnings: u32,
    notes: u32,
    color: bool,
}

impl Default for DiagnosticEngine {
    fn default() -> Self {
        let color = io::stderr().is_terminal();
        Self {
            errors: 0,
            warnings: 0,
            notes: 0,
            color,
        }
    }
}

impl DiagnosticEngine {
    /// Creates an engine that colors diagnostics when stderr is a terminal.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Overrides automatic color detection.
    pub fn set_color(&mut self, enabled: bool) {
        self.color = enabled;
    }

    /// Number of errors and fatal errors reported so far.
    #[must_use]
    pub const fn error_count(&self) -> u32 {
        self.errors
    }

    /// Number of warnings reported so far.
    #[must_use]
    pub const fn warning_count(&self) -> u32 {
        self.warnings
    }

    /// Number of notes reported so far.
    #[must_use]
    pub const fn note_count(&self) -> u32 {
        self.notes
    }

    /// Returns whether any error or fatal error has been reported.
    #[must_use]
    pub const fn has_errors(&self) -> bool {
        self.errors > 0
    }

    /// Formats and writes one diagnostic, updating the counters.
    ///
    /// # Errors
    ///
    /// Returns the sink's I/O error, if writing fails.
    pub fn report<W: Write>(
        &mut self,
        sink: &mut W,
        sources: &SourceManager,
        severity: Severity,
        span: Option<Span>,
        message: &str,
    ) -> io::Result<()> {
        match severity {
            Severity::Note => self.notes += 1,
            Severity::Warning => self.warnings += 1,
            Severity::Error | Severity::Fatal => self.errors += 1,
        }
        let text = render(sources, severity, span, message, self.color);
        sink.write_all(text.as_bytes())
    }

    /// Convenience wrapper that reports to the process's stderr, ignoring
    /// write errors (a broken stderr is not the compiler's problem).
    pub fn report_stderr(
        &mut self,
        sources: &SourceManager,
        severity: Severity,
        span: Option<Span>,
        message: &str,
    ) {
        let _ = self.report(&mut io::stderr(), sources, severity, span, message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::{FileId, SourceLocation};

    fn sample() -> (SourceManager, FileId) {
        let mut sources = SourceManager::new();
        let file = sources.add_buffer("sample.nova", "int x;\nint y;\n");
        (sources, file)
    }

    #[test]
    fn error_without_location_has_no_excerpt_or_caret() {
        let (sources, _) = sample();
        let text = render(&sources, Severity::Error, None, "something broke", false);

        assert_eq!(text, "error: something broke\n");
        assert!(!text.contains('^'));
    }

    #[test]
    fn error_with_span_prints_location_excerpt_and_caret() {
        let (sources, file) = sample();
        let span = Span::new(file, 11, 12);
        let text = render(
            &sources,
            Severity::Error,
            Some(span),
            "unknown identifier 'y'",
            false,
        );

        assert!(
            text.contains("sample.nova:2:5: error: unknown identifier 'y'"),
            "{text}"
        );
        assert!(text.contains("  int y;\n"), "{text}");
        assert!(
            text.contains("      ^\n"),
            "caret should sit under 'y': {text}"
        );
    }

    #[test]
    fn tabs_expand_for_caret_alignment() {
        let mut sources = SourceManager::new();
        let file = sources.add_buffer("tabs.nova", "\tvalue\n");
        let span = Span::new(file, 1, 6);
        let text = render(&sources, Severity::Warning, Some(span), "tab", false);

        assert!(text.contains("      value\n"), "{text}");
        assert!(text.contains("      ^"), "{text}");
    }

    #[test]
    fn engine_counts_and_renders_into_a_sink() {
        let (sources, file) = sample();
        let mut engine = DiagnosticEngine::new();
        engine.set_color(false);

        let mut sink = Vec::new();
        engine
            .report(
                &mut sink,
                &sources,
                Severity::Warning,
                Some(Span::new(file, 0, 3)),
                "unused",
            )
            .unwrap();
        engine
            .report(&mut sink, &sources, Severity::Error, None, "failed")
            .unwrap();

        let text = String::from_utf8(sink).unwrap();
        assert_eq!(engine.warning_count(), 1);
        assert_eq!(engine.error_count(), 1);
        assert!(engine.has_errors());
        assert!(text.contains("warning: unused"));
        assert!(text.contains("error: failed"));
    }

    #[test]
    fn colors_only_when_requested() {
        let (sources, _) = sample();
        let plain = render(&sources, Severity::Error, None, "boom", false);
        let colored = render(&sources, Severity::Error, None, "boom", true);

        assert_eq!(plain, "error: boom\n");
        assert!(colored.contains("\x1b["), "expected ANSI escapes");
    }

    #[test]
    fn unknown_span_renders_without_location() {
        let sources = SourceManager::new();
        let span = Some(Span::new(SourceLocation::default().file, 0, 1));
        let text = render(&sources, Severity::Error, span, "boom", false);
        assert_eq!(text, "error: boom\n");
    }
}
