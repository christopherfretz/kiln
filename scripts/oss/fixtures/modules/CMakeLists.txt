# The CMake C++20 modules fixture of selftest.sh (WP-982): an interface
# unit, an interface partition, an implementation unit and a consumer,
# built through FILE_SET CXX_MODULES with Ninja, which scans each source
# with the clang-scan-deps it finds beside the compiler (kiln-scan-deps).
# TODO(WP-984): a consumer that does `import std;`
# (CMAKE_CXX_MODULE_STD) once kiln builds the std module.
cmake_minimum_required(VERSION 3.28)
project(kiln_modules CXX)
set(CMAKE_CXX_STANDARD 20)
add_library(greet)
target_sources(greet
  PUBLIC FILE_SET CXX_MODULES FILES greet.cppm greet-part.cppm
  PRIVATE greet-impl.cpp)
# Accepted as a no-op by kiln (spec/driver.md "Dependency scanning").
target_compile_options(greet PRIVATE -fmodules-reduced-bmi)
add_executable(app main.cpp)
target_link_libraries(app PRIVATE greet)
