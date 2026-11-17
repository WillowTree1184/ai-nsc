#include "ainsc/Base/Diagnostics.h"

#include "ainsc/Base/SourceManager.h"

#include "llvm/ADT/StringRef.h"
#include "llvm/Support/ErrorHandling.h"

#include <string>

namespace ainsc {
namespace {

constexpr const char *kColorReset = "\033[0m";
constexpr const char *kColorRed = "\033[0;1;31m";
constexpr const char *kColorYellow = "\033[0;1;33m";
constexpr const char *kColorCyan = "\033[0;1;36m";
constexpr const char *kColorBold = "\033[1m";

constexpr unsigned kTabWidth = 4;

llvm::StringRef severityLabel(Severity severity) {
  switch (severity) {
  case Severity::Note:
    return "note";
  case Severity::Warning:
    return "warning";
  case Severity::Error:
    return "error";
  case Severity::Fatal:
    return "fatal error";
  }
  llvm_unreachable("unhandled Severity value");
}

const char *severityColor(Severity severity) {
  switch (severity) {
  case Severity::Note:
    return kColorCyan;
  case Severity::Warning:
    return kColorYellow;
  case Severity::Error:
  case Severity::Fatal:
    return kColorRed;
  }
  llvm_unreachable("unhandled Severity value");
}

} // namespace

DiagnosticEngine::DiagnosticEngine(SourceManager &sources, llvm::raw_ostream &os)
    : sources_(sources), os_(os) {}

void DiagnosticEngine::report(Severity severity, const llvm::Twine &message) {
  report(severity, SourceLocation(), message);
}

void DiagnosticEngine::report(Severity severity, SourceLocation location,
                              const llvm::Twine &message) {
  switch (severity) {
  case Severity::Note:
    ++noteCount_;
    break;
  case Severity::Warning:
    ++warningCount_;
    break;
  case Severity::Error:
  case Severity::Fatal:
    ++errorCount_;
    break;
  }

  const bool useColor = colorsEnabled_ && os_.has_colors();
  const bool hasLocation = location.isValid() && sources_.hasFile(location.file());

  unsigned column = 0;
  if (hasLocation) {
    const auto position = sources_.getLineAndColumn(location);
    column = position.second;
    os_ << sources_.path(location.file()) << ':' << position.first << ':' << column << ": ";
  }

  if (useColor) {
    os_ << severityColor(severity);
  }
  os_ << severityLabel(severity) << ": ";
  if (useColor) {
    os_ << kColorReset;
  }
  os_ << message << '\n';

  if (hasLocation) {
    printSourceLine(location, column);
  }
}

void DiagnosticEngine::printSourceLine(SourceLocation location, unsigned column) {
  unsigned lineNumber = 0;
  const llvm::StringRef text = sources_.getLine(location, lineNumber);
  if (lineNumber == 0) {
    return;
  }

  // Render the line with tabs expanded and remember where the caret belongs in
  // the rendered (visual) coordinates.
  std::string rendered;
  unsigned caretVisual = 0;
  bool caretPlaced = false;
  unsigned visual = 0;
  const unsigned targetByteIndex = column == 0 ? 0 : column - 1;
  for (unsigned i = 0; i < text.size(); ++i) {
    if (i == targetByteIndex) {
      caretVisual = visual;
      caretPlaced = true;
    }
    if (text[i] == '\t') {
      const unsigned spaces = kTabWidth - (visual % kTabWidth);
      rendered.append(spaces, ' ');
      visual += spaces;
    } else {
      rendered.push_back(text[i]);
      ++visual;
    }
  }
  if (!caretPlaced) {
    caretVisual = visual;
  }

  constexpr const char *kGutter = "  ";
  os_ << kGutter << rendered << '\n';
  os_ << kGutter;
  os_.indent(caretVisual);
  const bool useColor = colorsEnabled_ && os_.has_colors();
  if (useColor) {
    os_ << kColorBold;
  }
  os_ << '^';
  if (useColor) {
    os_ << kColorReset;
  }
  os_ << '\n';
}

} // namespace ainsc
