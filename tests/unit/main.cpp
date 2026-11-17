#include "Testing.h"

#include <cstddef>
#include <iostream>

namespace ainsc::test {

std::vector<TestCase> &registry() {
  static std::vector<TestCase> tests;
  return tests;
}

Registrar::Registrar(const char *suite, const char *name, TestFunction function) {
  registry().push_back(TestCase{suite, name, function});
}

void TestContext::failure(const char *file, int line, const std::string &message) {
  ++failures;
  std::cerr << "    " << file << ':' << line << ": " << message << '\n';
}

} // namespace ainsc::test

int main() {
  using ainsc::test::TestCase;
  using ainsc::test::TestContext;

  std::size_t failedTests = 0;
  for (const TestCase &test : ainsc::test::registry()) {
    TestContext context;
    std::cout << "[ RUN      ] " << test.suite << '.' << test.name << '\n';
    test.function(context);
    if (context.failures == 0) {
      std::cout << "[       OK ] " << test.suite << '.' << test.name << '\n';
    } else {
      ++failedTests;
      std::cout << "[  FAILED  ] " << test.suite << '.' << test.name << " (" << context.failures
                << " failure(s))\n";
    }
  }

  const std::size_t total = ainsc::test::registry().size();
  std::cout << '\n' << (total - failedTests) << '/' << total << " tests passed\n";
  return failedTests == 0 ? 0 : 1;
}
