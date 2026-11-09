# The CMake C++20 modules fixture of selftest.sh (WP-982): an interface
# unit, an interface partition, an implementation unit and a consumer,
# built through FILE_SET CXX_MODULES with Ninja, which scans each source
# with the clang-scan-deps it finds beside the compiler (kiln-scan-deps).
#
# WP-984: `stdapp` does `import std;` through CXX_MODULE_STD. CMake then
# asks the compiler for `-print-file-name=libc++.modules.json`, builds
# libc++'s std.cppm/std.compat.cppm as a target of its own, and gives the
# consumer the module file. That needs libc++ (selftest.sh configures
# with -DCMAKE_CXX_FLAGS=-stdlib=libc++), C++23, and CMake's experimental
# gate, a per-release UUID set before project(). Tested with 3.31; the
# later values are from each release's Help/dev/experimental.rst.
cmake_minimum_required(VERSION 3.28)
if(CMAKE_VERSION VERSION_LESS 4.0)
  set(CMAKE_EXPERIMENTAL_CXX_IMPORT_STD "0e5b6991-d74f-4b3d-a41c-cf096e0b2508")
elseif(CMAKE_VERSION VERSION_LESS 4.1)
  set(CMAKE_EXPERIMENTAL_CXX_IMPORT_STD "a9e1cf81-9932-4810-974b-6eccaf14e457")
else()
  set(CMAKE_EXPERIMENTAL_CXX_IMPORT_STD "d0edc3af-4c50-42ea-a356-e2862fe7a444")
endif()
project(kiln_modules CXX)
set(CMAKE_CXX_STANDARD 20)
add_library(greet)
target_sources(greet
  PUBLIC FILE_SET CXX_MODULES FILES greet.cppm greet-part.cppm
  PRIVATE greet-impl.cpp)
# Accepted as a no-op by kiln (spec/driver.md "Dependency scanning").
# clang 19 (the harness's control compiler, oss-build.sh cmake_modules)
# does not know it.
if(NOT CMAKE_CXX_COMPILER_VERSION VERSION_LESS 20)
  target_compile_options(greet PRIVATE -fmodules-reduced-bmi)
endif()
add_executable(app main.cpp)
target_link_libraries(app PRIVATE greet)
if("23" IN_LIST CMAKE_CXX_COMPILER_IMPORT_STD)
  add_executable(stdapp stdmain.cpp)
  set_target_properties(stdapp PROPERTIES CXX_STANDARD 23 CXX_MODULE_STD ON)
else()
  # selftest.sh fails its stdapp checks; say why here.
  message(WARNING "no import std: ${CMAKE_CXX_COMPILER_IMPORT_STD_NOT_FOUND_MESSAGE_23}")
endif()
