#include "Testing.h"

#include "ainsc/Base/Diagnostics.h"
#include "ainsc/Base/SourceManager.h"

#include "llvm/Support/raw_ostream.h"

#include <string>

using namespace ainsc;

namespace {

/// A self-contained diagnostic sink: source manager, in-memory stream and the
/// engine writing into it.
struct Captured {
  SourceManager sources;
  std::string text;
  llvm::raw_string_ostream stream{text};
  DiagnosticEngine diagnostics{sources, stream};
};

} // namespace

AINSC_TEST(Diagnostics, ErrorWithoutLocation) {
  Captured captured;
  captured.diagnostics.reportError("something broke");
  captured.stream.flush();

  AINSC_CHECK(captured.diagnostics.hasErrors());
  AINSC_CHECK_EQ(captured.diagnostics.errorCount(), 1u);
  AINSC_CHECK(captured.text.find("error: something broke") != std::string::npos);
  // No source excerpt and no caret when there is no location.
  AINSC_CHECK(captured.text.find('^') == std::string::npos);
}

AINSC_TEST(Diagnostics, ErrorWithLocationPrintsExcerptAndCaret) {
  Captured captured;
  const FileID file = captured.sources.addBuffer("sample.nova", "int x;\nint y;\n");
  captured.diagnostics.reportError(SourceLocation(file, 11), "unknown identifier 'y'");
  captured.stream.flush();

  AINSC_CHECK(captured.text.find("sample.nova:2:5: error: unknown identifier 'y'") !=
              std::string::npos);
  AINSC_CHECK(captured.text.find("int y;") != std::string::npos);
  AINSC_CHECK(captured.text.find('^') != std::string::npos);
}

AINSC_TEST(Diagnostics, WarningsDoNotFailCompilation) {
  Captured captured;
  const FileID file = captured.sources.addBuffer("sample.nova", "int x;\n");
  captured.diagnostics.report(Severity::Warning, SourceLocation(file, 0),
                              "unused variable 'x'");
  captured.stream.flush();

  AINSC_CHECK(!captured.diagnostics.hasErrors());
  AINSC_CHECK_EQ(captured.diagnostics.errorCount(), 0u);
  AINSC_CHECK_EQ(captured.diagnostics.warningCount(), 1u);
  AINSC_CHECK(captured.text.find("warning: unused variable 'x'") != std::string::npos);
}

AINSC_TEST(Diagnostics, CountsAccumulate) {
  Captured captured;
  const FileID file = captured.sources.addBuffer("sample.nova", "int x;\n");

  captured.diagnostics.reportNote(SourceLocation(file, 0), "declared here");
  captured.diagnostics.report(Severity::Warning, SourceLocation(file, 0), "first warning");
  captured.diagnostics.report(Severity::Error, SourceLocation(file, 0), "first error");
  captured.diagnostics.report(Severity::Fatal, "cannot continue");

  AINSC_CHECK_EQ(captured.diagnostics.errorCount(), 2u);
  AINSC_CHECK_EQ(captured.diagnostics.warningCount(), 1u);
  AINSC_CHECK(captured.diagnostics.hasErrors());
}
