#pragma once

#include <cstdint>

namespace ainsc {

/// Identifies a source file loaded into a SourceManager.
///
/// FileID 0 is the first file loaded. kInvalidFileID marks "no file", which is
/// what a default-constructed SourceLocation carries.
using FileID = std::uint32_t;

inline constexpr FileID kInvalidFileID = ~FileID{0};

/// A position inside a source file: a file id plus a byte offset.
///
/// Offsets are byte offsets into the *original* buffer; tokens carry one of
/// these instead of a line/column pair so that a position survives edits and
/// stays cheap to copy. Line/column mapping is done on demand by the
/// SourceManager.
class SourceLocation {
public:
  constexpr SourceLocation() = default;
  constexpr SourceLocation(FileID file, std::uint32_t offset) : file_(file), offset_(offset) {}

  constexpr FileID file() const { return file_; }
  constexpr std::uint32_t offset() const { return offset_; }
  constexpr bool isValid() const { return file_ != kInvalidFileID; }

  friend constexpr bool operator==(SourceLocation lhs, SourceLocation rhs) {
    return lhs.file_ == rhs.file_ && lhs.offset_ == rhs.offset_;
  }
  friend constexpr bool operator!=(SourceLocation lhs, SourceLocation rhs) {
    return !(lhs == rhs);
  }
  friend constexpr bool operator<(SourceLocation lhs, SourceLocation rhs) {
    return lhs.file_ != rhs.file_ ? lhs.file_ < rhs.file_ : lhs.offset_ < rhs.offset_;
  }

private:
  FileID file_ = kInvalidFileID;
  std::uint32_t offset_ = 0;
};

/// A half-open byte range [begin, end) inside a single source file.
struct SourceRange {
  SourceLocation begin;
  SourceLocation end;

  constexpr SourceRange() = default;
  constexpr SourceRange(SourceLocation beginLocation, SourceLocation endLocation)
      : begin(beginLocation), end(endLocation) {}

  constexpr bool isValid() const { return begin.isValid() && end.isValid(); }

  /// Number of bytes covered by the range (0 for invalid or multi-file ranges).
  constexpr std::uint32_t length() const {
    if (!isValid() || begin.file() != end.file() || end.offset() < begin.offset()) {
      return 0;
    }
    return end.offset() - begin.offset();
  }

  friend constexpr bool operator==(SourceRange lhs, SourceRange rhs) {
    return lhs.begin == rhs.begin && lhs.end == rhs.end;
  }
};

} // namespace ainsc
