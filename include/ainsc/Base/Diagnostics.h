#pragma once

#include "ainsc/Base/SourceLocation.h"

#include "llvm/ADT/Twine.h"
#include "llvm/Support/raw_ostream.h"

namespace ainsc {

class SourceManager;

/// Severity of a diagnostic. Errors and fatal errors make compilation fail;
/// warnings and notes do not.
enum class Severity {
  Note,
  Warning,
  Error,
  Fatal,
};

/// Collects diagnostics, prints them in a `file:line:col: severity: message`
/// format followed by the offending source line and a caret, and keeps counts
/// for the driver.
///
/// The engine deliberately owns no policy: it never aborts, never throws and
/// never decides whether a diagnostic is fatal. Front-end phases report what
/// they find and the driver inspects hasErrors() afterwards.
class DiagnosticEngine {
public:
  DiagnosticEngine(SourceManager &sources, llvm::raw_ostream &os);

  /// Diagnostics are plain text unless colours are enabled *and* the stream
  /// supports them (e.g. a tty).
  void setColorsEnabled(bool enabled) { colorsEnabled_ = enabled; }
  bool colorsEnabled() const { return colorsEnabled_; }

  void report(Severity severity, const llvm::Twine &message);
  void report(Severity severity, SourceLocation location, const llvm::Twine &message);

  void reportNote(SourceLocation location, const llvm::Twine &message) {
    report(Severity::Note, location, message);
  }
  void reportWarning(SourceLocation location, const llvm::Twine &message) {
    report(Severity::Warning, location, message);
  }
  void reportError(SourceLocation location, const llvm::Twine &message) {
    report(Severity::Error, location, message);
  }
  void reportError(const llvm::Twine &message) { report(Severity::Error, message); }

  unsigned errorCount() const { return errorCount_; }
  unsigned warningCount() const { return warningCount_; }
  bool hasErrors() const { return errorCount_ > 0; }

private:
  void printSourceLine(SourceLocation location, unsigned column);

  SourceManager &sources_;
  llvm::raw_ostream &os_;
  bool colorsEnabled_ = false;
  unsigned errorCount_ = 0;
  unsigned warningCount_ = 0;
  unsigned noteCount_ = 0;
};

} // namespace ainsc
