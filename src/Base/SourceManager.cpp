#include "ainsc/Base/SourceManager.h"

#include "llvm/Support/MemoryBuffer.h"

#include <algorithm>
#include <cassert>
#include <system_error>

namespace ainsc {
namespace {

std::uint32_t clampOffset(std::uint32_t offset, const std::string &contents) {
  const auto size = static_cast<std::uint32_t>(contents.size());
  return std::min(offset, size);
}

} // namespace

FileID SourceManager::addBuffer(llvm::StringRef path, std::string contents) {
  File file;
  file.path = path.str();
  file.contents = std::move(contents);
  file.lineStarts = computeLineStarts(file.contents);
  files_.push_back(std::move(file));
  return static_cast<FileID>(files_.size() - 1);
}

std::optional<FileID> SourceManager::loadFile(llvm::StringRef path, std::string &error) {
  auto bufferOrError = llvm::MemoryBuffer::getFile(path);
  if (!bufferOrError) {
    error = "cannot read '" + path.str() + "': " + bufferOrError.getError().message();
    return std::nullopt;
  }
  return addBuffer(path, (*bufferOrError)->getBuffer().str());
}

const std::string &SourceManager::path(FileID id) const {
  assert(hasFile(id) && "SourceManager::path: unknown FileID");
  return files_[id].path;
}

llvm::StringRef SourceManager::buffer(FileID id) const {
  if (!hasFile(id)) {
    return {};
  }
  return files_[id].contents;
}

std::vector<std::uint32_t> SourceManager::computeLineStarts(llvm::StringRef text) {
  std::vector<std::uint32_t> starts;
  starts.push_back(0);
  for (std::uint32_t i = 0; i < text.size(); ++i) {
    if (text[i] == '\n') {
      starts.push_back(i + 1);
    }
  }
  return starts;
}

std::pair<unsigned, unsigned> SourceManager::getLineAndColumn(SourceLocation loc) const {
  if (!hasFile(loc.file())) {
    return {0, 0};
  }
  const File &file = files_[loc.file()];
  const std::uint32_t offset = clampOffset(loc.offset(), file.contents);

  // upper_bound finds the first line start strictly greater than the offset;
  // its distance from begin() is the 1-based line number.
  const auto lineIt = std::upper_bound(file.lineStarts.begin(), file.lineStarts.end(), offset);
  const auto line = static_cast<unsigned>(std::distance(file.lineStarts.begin(), lineIt));
  const std::uint32_t lineStart = file.lineStarts[line - 1];
  const auto column = static_cast<unsigned>(offset - lineStart + 1);
  return {line, column};
}

unsigned SourceManager::getLineNumber(SourceLocation loc) const {
  return getLineAndColumn(loc).first;
}

llvm::StringRef SourceManager::getLine(SourceLocation loc, unsigned &lineNumber) const {
  lineNumber = 0;
  if (!hasFile(loc.file())) {
    return {};
  }
  const File &file = files_[loc.file()];
  const std::uint32_t offset = clampOffset(loc.offset(), file.contents);

  const auto lineIt = std::upper_bound(file.lineStarts.begin(), file.lineStarts.end(), offset);
  const auto line = static_cast<std::size_t>(std::distance(file.lineStarts.begin(), lineIt));
  const std::uint32_t begin = file.lineStarts[line - 1];
  const std::uint32_t end = line < file.lineStarts.size()
                                ? file.lineStarts[line]
                                : static_cast<std::uint32_t>(file.contents.size());

  std::uint32_t textEnd = end;
  while (textEnd > begin && (file.contents[textEnd - 1] == '\n' || file.contents[textEnd - 1] == '\r')) {
    --textEnd;
  }

  lineNumber = static_cast<unsigned>(line);
  return llvm::StringRef(file.contents).substr(begin, textEnd - begin);
}

} // namespace ainsc
