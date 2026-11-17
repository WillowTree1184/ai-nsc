#pragma once

#include "ainsc/Base/Diagnostics.h"
#include "ainsc/Base/SourceManager.h"

#include "llvm/ADT/StringRef.h"

#include <optional>
#include <string>
#include <vector>

namespace ainsc {

/// What the user asked the compiler to do.
enum class Action {
  None,
  Help,
  Version,
  /// Parse and type-check only; no code generation.
  Check,
  /// Emit LLVM IR (textual .ll).
  EmitIR,
  /// Emit a native object file (.o).
  EmitObj,
  /// Compile and link an executable.
  Build,
  /// Compile, link, then execute the program.
  Run,
};

/// Fully parsed command line.
struct DriverOptions {
  Action action = Action::None;
  std::vector<std::string> inputs;
  std::string output;
  std::string targetTriple;
  unsigned optimizationLevel = 0;
  bool debugInfo = false;
  bool dumpTokens = false;
  bool dumpAst = false;
  bool colorDiagnostics = true;
};

/// The ai-nsc command line driver.
///
/// Ownership: the Driver owns the SourceManager and the DiagnosticEngine for
/// the whole compilation, so every phase receives references to them. run() is
/// the only public entry point.
class Driver {
public:
  Driver();

  /// Parses the command line, runs the requested action, and returns the
  /// process exit code (0 on success).
  int run(int argc, const char *const *argv);

private:
  /// Returns nullopt to continue, or an exit code when parsing already
  /// produced a terminal result (e.g. a usage error).
  std::optional<int> parseArguments(int argc, const char *const *argv);

  std::optional<Action> commandFromName(llvm::StringRef name) const;

  void printVersion() const;
  void printUsage() const;

  /// Reports a bad invocation on stderr and returns exit code 1.
  std::optional<int> usageError(const llvm::Twine &message);

  /// Shared entry point for every action that consumes source files.
  int compile(const DriverOptions &options);

  DriverOptions options_;
  SourceManager sources_;
  DiagnosticEngine diagnostics_;
};

} // namespace ainsc
