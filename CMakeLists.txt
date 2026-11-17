cmake_minimum_required(VERSION 3.20)

project(ai-nsc
  VERSION 0.1.0
  DESCRIPTION "NovaSharp compiler with an LLVM backend"
  LANGUAGES C CXX)

if(CMAKE_SOURCE_DIR STREQUAL CMAKE_BINARY_DIR)
  message(FATAL_ERROR
    "ai-nsc: in-source builds are not supported; use a preset, e.g. `cmake --preset dev`.")
endif()

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)
set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

list(APPEND CMAKE_MODULE_PATH "${CMAKE_CURRENT_SOURCE_DIR}/cmake")
include(AinscWarnings)

# ---------------------------------------------------------------------------
# Options
# ---------------------------------------------------------------------------
option(AINSC_BUILD_TESTS "Build the ai-nsc unit tests" ON)
option(AINSC_ENABLE_WERROR "Treat compiler warnings as errors" OFF)
option(AINSC_ENABLE_ASAN "Build with AddressSanitizer and UBSan" OFF)

if(AINSC_ENABLE_ASAN)
  add_compile_options(-fsanitize=address -fsanitize=undefined -fno-omit-frame-pointer)
  add_link_options(-fsanitize=address -fsanitize=undefined)
endif()

# ---------------------------------------------------------------------------
# LLVM
# ---------------------------------------------------------------------------
set(AINSC_LLVM_DIR "/usr/lib/llvm-18/lib/cmake/llvm"
    CACHE PATH "Directory that contains LLVMConfig.cmake")
find_package(LLVM REQUIRED CONFIG PATHS "${AINSC_LLVM_DIR}" NO_DEFAULT_PATH)
message(STATUS "ai-nsc: using LLVM ${LLVM_PACKAGE_VERSION} (${AINSC_LLVM_DIR})")

# Single interface target that carries every LLVM include path, definition and
# link dependency. Front end and back end both consume it; nothing links LLVM
# directly.
add_library(ainsc_llvm INTERFACE)
target_include_directories(ainsc_llvm SYSTEM INTERFACE ${LLVM_INCLUDE_DIRS})
separate_arguments(AINSC_LLVM_DEFINITIONS NATIVE_COMMAND "${LLVM_DEFINITIONS}")
target_compile_options(ainsc_llvm INTERFACE ${AINSC_LLVM_DEFINITIONS})
if(TARGET LLVM)
  # Ubuntu/Debian LLVM ships with LLVM_LINK_LLVM_DYLIB=ON: link one shared
  # library instead of the ~225 static component libraries.
  target_link_libraries(ainsc_llvm INTERFACE LLVM)
else()
  llvm_map_components_to_libnames(AINSC_LLVM_LIBS core support irreader passes)
  target_link_libraries(ainsc_llvm INTERFACE ${AINSC_LLVM_LIBS})
endif()

# ---------------------------------------------------------------------------
# Generated headers
# ---------------------------------------------------------------------------
configure_file(
  "${CMAKE_CURRENT_SOURCE_DIR}/include/ainsc/Config.h.in"
  "${CMAKE_CURRENT_BINARY_DIR}/include/ainsc/Config.h"
  @ONLY)

# ---------------------------------------------------------------------------
# Core library: everything except main()
# ---------------------------------------------------------------------------
add_library(ainsc_core STATIC
  src/Base/Diagnostics.cpp
  src/Base/SourceManager.cpp
  src/Driver/Driver.cpp)

target_include_directories(ainsc_core
  PUBLIC
    "${CMAKE_CURRENT_SOURCE_DIR}/include"
    "${CMAKE_CURRENT_BINARY_DIR}/include")

target_link_libraries(ainsc_core PUBLIC ainsc_llvm)
ainsc_set_warnings(ainsc_core)

# ---------------------------------------------------------------------------
# Compiler executable
# ---------------------------------------------------------------------------
add_executable(ai-nsc src/main.cpp)
target_link_libraries(ai-nsc PRIVATE ainsc_core)
ainsc_set_warnings(ai-nsc)

# ---------------------------------------------------------------------------
# Install
# ---------------------------------------------------------------------------
include(GNUInstallDirs)
install(TARGETS ai-nsc RUNTIME DESTINATION "${CMAKE_INSTALL_BINDIR}")

# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------
if(AINSC_BUILD_TESTS)
  enable_testing()
  add_subdirectory(tests)
endif()
