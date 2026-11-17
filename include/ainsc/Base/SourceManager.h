#pragma once

#include "ainsc/Base/SourceLocation.h"

#include "llvm/ADT/StringRef.h"

#include <cstddef>
#include <cstdint>
#include <optional>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

namespace ainsc {

/// Owns every source buffer the compiler currently has open and provides
/// offset <-> line/column mapping.
///
/// Buffers are stored verbatim (no tab expansion, no newline translation) so
/// that diagnostics and the lexer see exactly the bytes on disk.
class SourceManager {
public:
  /// Registers a file with contents that are already in memory.
  FileID addBuffer(llvm::StringRef path, std::string contents);

  /// Reads @p path from disk and registers it.
  ///
  /// On failure returns std::nullopt and fills @p error with a human-readable
  /// message (without a trailing newline).
  std::optional<FileID> loadFile(llvm::StringRef path, std::string &error);

  bool hasFile(FileID id) const { return id < files_.size(); }
  std::size_t numFiles() const { return files_.size(); }

  /// Path exactly as it was given to addBuffer/loadFile.
  const std::string &path(FileID id) const;

  /// Full contents of the file.
  llvm::StringRef buffer(FileID id) const;

  /// 1-based (line, column) for the position of @p loc. Returns (0, 0) if the
  /// location does not belong to a known file.
  std::pair<unsigned, unsigned> getLineAndColumn(SourceLocation loc) const;

  /// 1-based line number for @p loc, or 0 if unknown.
  unsigned getLineNumber(SourceLocation loc) const;

  /// Text of the line containing @p loc, without the trailing newline.
  ///
  /// On success @p lineNumber receives the 1-based line number; on failure it
  /// receives 0 and the returned slice is empty.
  llvm::StringRef getLine(SourceLocation loc, unsigned &lineNumber) const;

  /// Convenience helper: a location at byte @p offset of @p file.
  static SourceLocation locationAt(FileID file, std::uint32_t offset) {
    return SourceLocation(file, offset);
  }

private:
  struct File {
    std::string path;
    std::string contents;
    std::vector<std::uint32_t> lineStarts;
  };

  static std::vector<std::uint32_t> computeLineStarts(llvm::StringRef text);

  std::vector<File> files_;
};

} // namespace ainsc
