#pragma once

// A tiny, dependency-free unit test harness.
//
// Keeping the harness in-tree means the unit tests stay buildable on a bare
// machine (no GoogleTest install required). If the project later grows a need
// for fixtures, parameterized tests or death tests, swapping this for
// GoogleTest only touches this file and the test sources' includes.

#include <sstream>
#include <string>
#include <vector>

namespace ainsc::test {

struct TestContext {
  int failures = 0;

  void failure(const char *file, int line, const std::string &message);
};

using TestFunction = void (*)(TestContext &);

struct TestCase {
  const char *suite;
  const char *name;
  TestFunction function;
};

/// All registered tests, in registration order (i.e. link order).
std::vector<TestCase> &registry();

struct Registrar {
  Registrar(const char *suite, const char *name, TestFunction function);
};

} // namespace ainsc::test

#define AINSC_DETAIL_CAT_IMPL(lhs, rhs) lhs##rhs
#define AINSC_DETAIL_CAT(lhs, rhs) AINSC_DETAIL_CAT_IMPL(lhs, rhs)
#define AINSC_DETAIL_TEST_NAME(suite, name) AINSC_DETAIL_CAT(suite, AINSC_DETAIL_CAT(_, name))

#define AINSC_TEST(suite, name)                                                          \
  static void AINSC_DETAIL_TEST_NAME(suite, name)(::ainsc::test::TestContext & ctx);     \
  [[maybe_unused]] static const ::ainsc::test::Registrar                                 \
      AINSC_DETAIL_CAT(kRegistrar_, AINSC_DETAIL_TEST_NAME(suite, name))(                \
          #suite, #name, &AINSC_DETAIL_TEST_NAME(suite, name));                          \
  static void AINSC_DETAIL_TEST_NAME(suite, name)(::ainsc::test::TestContext & ctx)

#define AINSC_CHECK(condition)                                                           \
  do {                                                                                   \
    if (!(condition)) {                                                                  \
      std::ostringstream ainsc_oss;                                                      \
      ainsc_oss << "CHECK failed: " #condition;                                          \
      ctx.failure(__FILE__, __LINE__, ainsc_oss.str());                                  \
    }                                                                                    \
  } while (false)

#define AINSC_CHECK_EQ(lhs, rhs)                                                         \
  do {                                                                                   \
    const auto &ainsc_lhs = (lhs);                                                       \
    const auto &ainsc_rhs = (rhs);                                                       \
    if (!(ainsc_lhs == ainsc_rhs)) {                                                     \
      std::ostringstream ainsc_oss;                                                      \
      ainsc_oss << "CHECK_EQ failed: " #lhs " == " #rhs;                                 \
      ctx.failure(__FILE__, __LINE__, ainsc_oss.str());                                  \
    }                                                                                    \
  } while (false)

#define AINSC_CHECK_NE(lhs, rhs) AINSC_CHECK(!((lhs) == (rhs)))
