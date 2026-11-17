#include "ainsc/Driver/Driver.h"

#include "ainsc/Config.h"

#include "llvm/ADT/StringRef.h"
#include "llvm/Support/Process.h"
#include "llvm/Support/raw_ostream.h"
#include "llvm/TargetParser/Host.h"

#include <cstddef>
#include <optional>
#include <string>

namespace ainsc {

Driver::Driver() : diagnostics_(sources_, llvm::errs()) {
  diagnostics_.setColorsEnabled(llvm::sys::Process::StandardErrHasColors());
}

int Driver::run(int argc, const char *const *argv) {
  if (auto exitCode = parseArguments(argc, argv)) {
    return *exitCode;
  }

  switch (options_.action) {
  case Action::Help:
    printUsage();
    return 0;
  case Action::Version:
    printVersion();
    return 0;
  case Action::None:
    printUsage();
    return 1;
  case Action::Check:
  case Action::EmitIR:
  case Action::EmitObj:
  case Action::Build:
  case Action::Run:
    return compile(options_);
  }
  return 1;
}

std::optional<int> Driver::parseArguments(int argc, const char *const *argv) {
  bool onlyInputs = false;
  for (int i = 1; i < argc; ++i) {
    const llvm::StringRef argument(argv[i]);
    if (onlyInputs) {
      options_.inputs.push_back(argument.str());
      continue;
    }

    if (argument == "--") {
      onlyInputs = true;
      continue;
    }
    if (argument == "-h" || argument == "--help") {
      options_.action = Action::Help;
      continue;
    }
    if (argument == "--version") {
      options_.action = Action::Version;
      continue;
    }
    if (argument == "-o") {
      if (i + 1 >= argc) {
        return usageError("missing value for '-o'");
      }
      options_.output = argv[++i];
      continue;
    }
    if (argument == "--target") {
      if (i + 1 >= argc) {
        return usageError("missing value for '--target'");
      }
      options_.targetTriple = argv[++i];
      continue;
    }
    if (argument.starts_with("--target=")) {
      options_.targetTriple = argument.drop_front(std::string("--target=").size()).str();
      continue;
    }
    if (argument == "-g") {
      options_.debugInfo = true;
      continue;
    }
    if (argument == "--dump-tokens") {
      options_.dumpTokens = true;
      continue;
    }
    if (argument == "--dump-ast") {
      options_.dumpAst = true;
      continue;
    }
    if (argument == "--no-color") {
      options_.colorDiagnostics = false;
      diagnostics_.setColorsEnabled(false);
      continue;
    }
    if (argument.starts_with("-O")) {
      if (argument.size() != 3 || argument[2] < '0' || argument[2] > '3') {
        return usageError("invalid optimization level '" + argument.str() +
                          "'; expected -O0, -O1, -O2 or -O3");
      }
      options_.optimizationLevel = static_cast<unsigned>(argument[2] - '0');
      continue;
    }
    if (argument.starts_with("-")) {
      return usageError("unknown option '" + argument.str() + "'");
    }

    // First positional argument may name a command; everything after it is an
    // input file.
    if (std::optional<Action> action = commandFromName(argument)) {
      options_.action = *action;
      continue;
    }
    options_.inputs.push_back(argument.str());
  }

  if (options_.action == Action::None) {
    options_.action = options_.inputs.empty() ? Action::Help : Action::Build;
  }
  return std::nullopt;
}

std::optional<Action> Driver::commandFromName(llvm::StringRef name) const {
  if (name == "build") {
    return Action::Build;
  }
  if (name == "run") {
    return Action::Run;
  }
  if (name == "check") {
    return Action::Check;
  }
  if (name == "emit-ir") {
    return Action::EmitIR;
  }
  if (name == "emit-obj") {
    return Action::EmitObj;
  }
  return std::nullopt;
}

void Driver::printVersion() const {
  llvm::outs() << "ai-nsc version " << AINSC_VERSION_STRING << '\n';
  llvm::outs() << "  LLVM: " << AINSC_LLVM_VERSION_STRING << '\n';
  llvm::outs() << "  default target: " << llvm::sys::getDefaultTargetTriple() << '\n';
}

void Driver::printUsage() const {
  llvm::outs()
      << "ai-nsc - the NovaSharp compiler (LLVM backend)\n"
      << "\n"
      << "USAGE:\n"
      << "  ai-nsc <command> [options] <file.nova>...\n"
      << "\n"
      << "COMMANDS:\n"
      << "  build     Compile NovaSharp sources and link an executable\n"
      << "  run       Compile and immediately run the program\n"
      << "  check     Parse and type-check only; no code generation\n"
      << "  emit-ir   Emit LLVM IR as textual .ll\n"
      << "  emit-obj  Emit a native object file (.o)\n"
      << "\n"
      << "OPTIONS:\n"
      << "  -o <path>           Output path\n"
      << "  --target <triple>   Target triple (default: host)\n"
      << "  -O0/-O1/-O2/-O3     Optimization level (default: -O0)\n"
      << "  -g                  Emit debug information\n"
      << "  --dump-tokens       Print the token stream\n"
      << "  --dump-ast          Print the AST after parsing\n"
      << "  --no-color          Disable colored diagnostics\n"
      << "  -h, --help          Show this help\n"
      << "  --version           Show version information\n";
}

std::optional<int> Driver::usageError(const llvm::Twine &message) {
  llvm::errs() << "ai-nsc: " << message << '\n';
  llvm::errs() << "Run 'ai-nsc --help' for usage.\n";
  return 1;
}

int Driver::compile(const DriverOptions &options) {
  if (options.inputs.empty()) {
    return *usageError("no input files");
  }

  bool allLoaded = true;
  for (const std::string &input : options.inputs) {
    std::string error;
    if (!sources_.loadFile(input, error)) {
      llvm::errs() << "ai-nsc: " << error << '\n';
      allLoaded = false;
    }
  }
  if (!allLoaded) {
    return 1;
  }

  // The front end (lexer, parser, semantic analysis) and the LLVM back end are
  // added once the NovaSharp grammar specification is available. Until then the
  // driver validates its own plumbing and reports why it cannot go further.
  diagnostics_.report(Severity::Error,
                      "the NovaSharp front end is not implemented yet: the language "
                      "grammar specification is still pending");
  return 1;
}

} // namespace ainsc
