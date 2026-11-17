//! Source files, locations and ranges.
//!
//! Tokens and AST nodes never store line/column pairs: they store a [`Span`], a
//! `FileId` plus byte offsets. Mapping to lines and columns happens on demand in
//! [`SourceManager`], which keeps copying cheap and lets any later source
//! transformation stay consistent.

use std::io;
use std::path::{Path, PathBuf};

/// Identifies a file registered with a [`SourceManager`].
///
/// This is a newtype rather than a bare `u32` so that it cannot be confused
/// with an offset. File ids are dense and start at zero.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct FileId(u32);

impl FileId {
    /// The "no file" sentinel.
    pub const INVALID: FileId = FileId(u32::MAX);

    /// Returns whether this id refers to a real file.
    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.0 != u32::MAX
    }

    /// Returns the dense index of this file.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// Creates a file id from its dense index. Only for tests and internals.
    #[must_use]
    pub const fn from_index(index: u32) -> FileId {
        FileId(index)
    }
}

impl Default for FileId {
    fn default() -> Self {
        FileId::INVALID
    }
}

/// A position inside a source file: a file id and a byte offset.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct SourceLocation {
    /// File the location belongs to.
    pub file: FileId,
    /// Byte offset from the start of the file.
    pub offset: u32,
}

impl SourceLocation {
    /// Creates a location.
    #[must_use]
    pub const fn new(file: FileId, offset: u32) -> Self {
        Self { file, offset }
    }

    /// Returns whether this location points into a real file.
    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.file.is_valid()
    }
}

/// A half-open byte range `[start, end)` inside a single source file.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Span {
    /// File the span belongs to.
    pub file: FileId,
    /// First byte of the span.
    pub start: u32,
    /// One past the last byte of the span.
    pub end: u32,
}

impl Span {
    /// Creates a span.
    #[must_use]
    pub const fn new(file: FileId, start: u32, end: u32) -> Self {
        Self { file, start, end }
    }

    /// Creates a span covering exactly `start..start + len`.
    #[must_use]
    pub const fn with_len(file: FileId, start: u32, len: u32) -> Self {
        Self {
            file,
            start,
            end: start + len,
        }
    }

    /// Number of bytes covered by the span.
    #[must_use]
    pub const fn len(self) -> u32 {
        self.end.saturating_sub(self.start)
    }

    /// Returns whether the span covers no bytes.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.len() == 0
    }

    /// Returns whether the span refers to a real file.
    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.file.is_valid()
    }

    /// Location of the first byte of the span; the natural anchor for
    /// diagnostics.
    #[must_use]
    pub const fn start_location(self) -> SourceLocation {
        SourceLocation::new(self.file, self.start)
    }

    /// Returns whether `offset` falls inside the span.
    #[must_use]
    pub const fn contains(self, offset: u32) -> bool {
        self.start <= offset && offset < self.end
    }
}

/// One loaded source file: its path, its exact contents and a line index.
#[derive(Debug)]
pub struct SourceFile {
    path: PathBuf,
    text: String,
    line_starts: Vec<u32>,
}

impl SourceFile {
    fn new(path: PathBuf, text: String) -> Self {
        let line_starts = compute_line_starts(&text);
        Self {
            path,
            text,
            line_starts,
        }
    }

    /// Path exactly as it was registered.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Exact contents of the file.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Number of lines, counting a trailing empty line after a final newline.
    #[must_use]
    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    /// Index into `line_starts` for `offset`.
    fn line_index(&self, offset: u32) -> usize {
        let offset = offset.min(self.byte_len());
        self.line_starts
            .partition_point(|&start| start <= offset)
            .saturating_sub(1)
    }

    fn byte_len(&self) -> u32 {
        self.text.len() as u32
    }

    /// 1-based line and column for `offset`. Columns count bytes, so a
    /// multi-byte UTF-8 character advances the column by its byte length.
    #[must_use]
    pub fn line_col(&self, offset: u32) -> (u32, u32) {
        let offset = offset.min(self.byte_len());
        let line_index = self.line_index(offset);
        let line_start = self.line_starts[line_index];
        (line_index as u32 + 1, offset - line_start + 1)
    }

    /// Text of the line containing `offset`, without its trailing newline, and
    /// the 1-based line number.
    #[must_use]
    pub fn line_text(&self, offset: u32) -> (&str, u32) {
        let offset = offset.min(self.byte_len());
        let line_index = self.line_index(offset);
        let start = self.line_starts[line_index] as usize;
        let end = self
            .line_starts
            .get(line_index + 1)
            .map_or(self.text.len(), |&next| next as usize);

        let mut trimmed_end = end;
        while trimmed_end > start && matches!(self.text.as_bytes()[trimmed_end - 1], b'\n' | b'\r')
        {
            trimmed_end -= 1;
        }

        (&self.text[start..trimmed_end], line_index as u32 + 1)
    }
}

/// Computes the byte offset at which every line starts.
fn compute_line_starts(text: &str) -> Vec<u32> {
    let mut starts = vec![0];
    for (index, byte) in text.bytes().enumerate() {
        if byte == b'\n' {
            starts.push(index as u32 + 1);
        }
    }
    starts
}

/// Owns every source buffer the compiler has open and maps offsets to
/// line/column information.
#[derive(Debug, Default)]
pub struct SourceManager {
    files: Vec<SourceFile>,
}

impl SourceManager {
    /// Creates an empty manager.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers an in-memory buffer and returns its id.
    pub fn add_buffer(&mut self, path: impl Into<PathBuf>, text: impl Into<String>) -> FileId {
        let id = FileId::from_index(self.files.len() as u32);
        self.files.push(SourceFile::new(path.into(), text.into()));
        id
    }

    /// Reads `path` from disk and registers it.
    ///
    /// # Errors
    ///
    /// Returns the underlying I/O error if the file cannot be read. The error
    /// does not include the path; callers add it to their message.
    pub fn load_file(&mut self, path: impl AsRef<Path>) -> io::Result<FileId> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path)?;
        Ok(self.add_buffer(path.to_path_buf(), text))
    }

    /// Number of registered files.
    #[must_use]
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// Returns whether no file has been registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Returns the file for `id`, if it exists.
    #[must_use]
    pub fn file(&self, id: FileId) -> Option<&SourceFile> {
        self.files.get(id.index())
    }

    /// Returns the contents of `id`, if it exists.
    #[must_use]
    pub fn text(&self, id: FileId) -> Option<&str> {
        self.file(id).map(SourceFile::text)
    }

    /// Returns the path of `id`, if it exists.
    #[must_use]
    pub fn path(&self, id: FileId) -> Option<&Path> {
        self.file(id).map(SourceFile::path)
    }

    /// 1-based line and column for `location`, if the file is known.
    #[must_use]
    pub fn line_col(&self, location: SourceLocation) -> Option<(u32, u32)> {
        self.file(location.file)
            .map(|file| file.line_col(location.offset))
    }

    /// Line text and 1-based line number for `location`, if the file is known.
    #[must_use]
    pub fn line_text(&self, location: SourceLocation) -> Option<(&str, u32)> {
        self.file(location.file)
            .map(|file| file.line_text(location.offset))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_offsets_to_lines_and_columns() {
        let mut sources = SourceManager::new();
        let file = sources.add_buffer("sample.nova", "int x;\nint y;\n");

        assert_eq!(sources.line_col(SourceLocation::new(file, 0)), Some((1, 1)));
        assert_eq!(sources.line_col(SourceLocation::new(file, 7)), Some((2, 1)));
        assert_eq!(
            sources.line_col(SourceLocation::new(file, 11)),
            Some((2, 5))
        );
        // One past the end is the empty line after the trailing newline.
        assert_eq!(
            sources.line_col(SourceLocation::new(file, 14)),
            Some((3, 1))
        );
    }

    #[test]
    fn strips_newline_from_line_text() {
        let mut sources = SourceManager::new();
        let file = sources.add_buffer("sample.nova", "int x;\nint y;\r\n");

        let location = SourceLocation::new(file, 11);
        assert_eq!(
            sources.line_text(location),
            Some(("int y;", 2)),
            "CRLF should be stripped too"
        );
    }

    #[test]
    fn unknown_locations_are_safe() {
        let sources = SourceManager::new();
        assert_eq!(sources.line_col(SourceLocation::default()), None);
        assert_eq!(sources.line_text(SourceLocation::default()), None);
        assert!(sources.file(FileId::INVALID).is_none());
        assert!(sources.text(FileId::INVALID).is_none());
    }

    #[test]
    fn loads_files_from_disk() {
        let mut sources = SourceManager::new();
        let path = std::env::temp_dir().join(format!("ainsc-source-{}.nova", std::process::id()));
        std::fs::write(&path, "let answer = 42;\n").expect("write temp source");

        let file = sources.load_file(&path).expect("load temp source");
        assert_eq!(sources.text(file), Some("let answer = 42;\n"));
        assert_eq!(sources.path(file), Some(path.as_path()));

        std::fs::remove_file(&path).expect("remove temp source");
    }

    #[test]
    fn spans_are_half_open() {
        let file = FileId::from_index(0);
        let span = Span::new(file, 10, 14);
        assert_eq!(span.len(), 4);
        assert!(span.contains(10));
        assert!(span.contains(13));
        assert!(!span.contains(14));
        assert_eq!(span.start_location(), SourceLocation::new(file, 10));
        assert!(!Span::default().is_valid());
    }
}
