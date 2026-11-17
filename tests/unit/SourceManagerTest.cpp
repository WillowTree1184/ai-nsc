#include "Testing.h"

#include "ainsc/Base/SourceManager.h"

#include "llvm/ADT/SmallString.h"
#include "llvm/ADT/StringRef.h"
#include "llvm/Support/FileSystem.h"
#include "llvm/Support/raw_ostream.h"

using namespace ainsc;

AINSC_TEST(SourceManager, LineAndColumnMapping) {
  SourceManager sources;
  const FileID file = sources.addBuffer("sample.nova", "int x;\nint y;\n");
  AINSC_CHECK_EQ(sources.numFiles(), std::size_t{1});

  const auto first = sources.getLineAndColumn(SourceLocation(file, 0));
  AINSC_CHECK_EQ(first.first, 1u);
  AINSC_CHECK_EQ(first.second, 1u);

  // Offset 7 is the 'i' of "int y".
  const auto secondStart = sources.getLineAndColumn(SourceLocation(file, 7));
  AINSC_CHECK_EQ(secondStart.first, 2u);
  AINSC_CHECK_EQ(secondStart.second, 1u);

  // Offset 11 is the 'y' of "int y".
  const auto secondMiddle = sources.getLineAndColumn(SourceLocation(file, 11));
  AINSC_CHECK_EQ(secondMiddle.first, 2u);
  AINSC_CHECK_EQ(secondMiddle.second, 5u);

  // One past the end is the empty line after the trailing newline.
  const auto end = sources.getLineAndColumn(SourceLocation(file, 14));
  AINSC_CHECK_EQ(end.first, 3u);
  AINSC_CHECK_EQ(end.second, 1u);
}

AINSC_TEST(SourceManager, GetLineStripsNewline) {
  SourceManager sources;
  const FileID file = sources.addBuffer("sample.nova", "int x;\nint y;\r\n");

  unsigned lineNumber = 0;
  const llvm::StringRef line = sources.getLine(SourceLocation(file, 11), lineNumber);
  AINSC_CHECK_EQ(lineNumber, 2u);
  AINSC_CHECK_EQ(line, llvm::StringRef("int y;"));
}

AINSC_TEST(SourceManager, UnknownLocationIsSafe) {
  SourceManager sources;
  const auto position = sources.getLineAndColumn(SourceLocation());
  AINSC_CHECK_EQ(position.first, 0u);
  AINSC_CHECK_EQ(position.second, 0u);

  unsigned lineNumber = 0;
  AINSC_CHECK(sources.getLine(SourceLocation(), lineNumber).empty());
  AINSC_CHECK_EQ(lineNumber, 0u);
  AINSC_CHECK_EQ(sources.getLineNumber(SourceLocation()), 0u);
}

AINSC_TEST(SourceManager, BufferContents) {
  SourceManager sources;
  const FileID file = sources.addBuffer("sample.nova", "let answer = 42;\n");
  AINSC_CHECK_EQ(sources.path(file), std::string("sample.nova"));
  AINSC_CHECK_EQ(sources.buffer(file), llvm::StringRef("let answer = 42;\n"));
  AINSC_CHECK(!sources.buffer(kInvalidFileID).data());
  AINSC_CHECK(!sources.hasFile(kInvalidFileID));
}

AINSC_TEST(SourceManager, LoadFileFromDisk) {
  int fd = -1;
  llvm::SmallString<128> path;
  const std::error_code createError =
      llvm::sys::fs::createTemporaryFile("ainsc-source-manager", ".nova", fd, path);
  AINSC_CHECK(!createError);
  if (createError) {
    return;
  }

  {
    llvm::raw_fd_ostream stream(fd, /*shouldClose=*/true);
    stream << "let a = 1;\n";
  }

  SourceManager sources;
  std::string error;
  const std::optional<FileID> file = sources.loadFile(path, error);
  AINSC_CHECK(file.has_value());
  AINSC_CHECK(error.empty());
  if (file) {
    AINSC_CHECK_EQ(sources.buffer(*file), llvm::StringRef("let a = 1;\n"));
  }

  llvm::sys::fs::remove(path);
}

AINSC_TEST(SourceManager, LoadMissingFileReportsError) {
  SourceManager sources;
  std::string error;
  const std::optional<FileID> file = sources.loadFile("definitely-not-here.nova", error);
  AINSC_CHECK(!file.has_value());
  AINSC_CHECK(!error.empty());
  AINSC_CHECK_EQ(sources.numFiles(), std::size_t{0});
}
