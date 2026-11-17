add_executable(ainsc-unit-tests
  unit/main.cpp
  unit/DiagnosticsTest.cpp
  unit/LLVMSmokeTest.cpp
  unit/SourceManagerTest.cpp)

target_link_libraries(ainsc-unit-tests PRIVATE ainsc_core)
target_include_directories(ainsc-unit-tests PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}/unit")
ainsc_set_warnings(ainsc-unit-tests)

add_test(NAME unit COMMAND ainsc-unit-tests)

# Smoke tests for the driver's own command line handling.
add_test(NAME cli-version COMMAND ai-nsc --version)
set_tests_properties(cli-version PROPERTIES PASS_REGULAR_EXPRESSION "ai-nsc version")

add_test(NAME cli-help COMMAND ai-nsc --help)
set_tests_properties(cli-help PROPERTIES PASS_REGULAR_EXPRESSION "USAGE:")

add_test(NAME cli-missing-input
  COMMAND ai-nsc check "${CMAKE_CURRENT_BINARY_DIR}/missing-file.nova")
set_tests_properties(cli-missing-input PROPERTIES WILL_FAIL TRUE)

add_test(NAME cli-unknown-option COMMAND ai-nsc --not-a-real-option)
set_tests_properties(cli-unknown-option PROPERTIES WILL_FAIL TRUE)
