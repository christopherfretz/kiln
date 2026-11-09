# fmt_module (WP-1148): included after `project(FMT)` through
# -DCMAKE_PROJECT_FMT_INCLUDE. With FMT_MODULE=ON, {fmt} builds `fmt` from
# src/fmt.cc (`export module fmt;`) as a FILE_SET CXX_MODULES library, but
# defines no importer: test/CMakeLists.txt returns before its tests in
# module mode ("Testing is incompatible with build mode 'module'"), and
# upstream's test/module-test.cc no longer compiles against 12.1.0 with
# clang 19 either (make_format_args of rvalues, fmt::is_char, a visible
# detail namespace), so it is not built. This defers, to the end of the
# top-level CMakeLists.txt, the importer fmt-module-smoke (from
# fmt-module-smoke.cc beside this file: `import fmt;`, no {fmt} header,
# checking the formatted text), its CTest case, and the target
# `fmt_module_tests` that builds exactly it (and so the module library).
function(_kiln_fmt_module_tests)
  if(NOT TARGET fmt OR NOT FMT_MODULE)
    message(FATAL_ERROR "fmt-module-tests.cmake needs -DFMT_MODULE=ON")
  endif()
  add_executable(fmt-module-smoke "${CMAKE_CURRENT_FUNCTION_LIST_DIR}/fmt-module-smoke.cc")
  target_link_libraries(fmt-module-smoke PRIVATE fmt)
  add_test(NAME fmt-module-smoke COMMAND fmt-module-smoke)
  add_custom_target(fmt_module_tests DEPENDS fmt-module-smoke)
endfunction()

# Here, not in the deferred call: CTest files are only written for
# directories that enabled testing while they were configured.
enable_testing()
cmake_language(DEFER DIRECTORY "${CMAKE_SOURCE_DIR}" CALL _kiln_fmt_module_tests)
