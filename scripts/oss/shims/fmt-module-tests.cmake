# fmt_module (WP-1148): included after `project(FMT)` through
# -DCMAKE_PROJECT_FMT_INCLUDE. With FMT_MODULE=ON, {fmt} builds `fmt` from
# src/fmt.cc (`export module fmt;`) as a FILE_SET CXX_MODULES library, but
# test/CMakeLists.txt returns before defining any test ("Testing is
# incompatible with build mode 'module'"), so CMake's dependency scanning
# never reaches an importer. This defers, to the end of the top-level
# CMakeLists.txt (where test-main, gtest and fmt-header-only exist when
# FMT_TEST=ON), two importers of the module and their CTest cases:
#   module-test       upstream's test/module-test.cc (gtest, `import fmt;`),
#                     linked to the module library and to a copy of
#                     test-main built against fmt-header-only (test-main
#                     itself links the module library, whose symbols are
#                     attached to `fmt`, so its #include users cannot link);
#   fmt-module-smoke  fmt-module-smoke.cc beside this file: `import fmt;`
#                     only, checking the formatted text.
# The aggregate target `fmt_module_tests` builds exactly them.
function(_kiln_fmt_module_tests)
  if(NOT TARGET gtest OR NOT TARGET fmt-header-only)
    message(FATAL_ERROR "fmt-module-tests.cmake needs -DFMT_TEST=ON -DFMT_MODULE=ON")
  endif()
  set(t "${CMAKE_SOURCE_DIR}/test")
  add_library(kiln-fmt-test-main STATIC "${t}/test-main.cc" "${t}/gtest-extra.cc" "${t}/util.cc")
  target_link_libraries(kiln-fmt-test-main PUBLIC gtest fmt-header-only)
  set_target_properties(kiln-fmt-test-main PROPERTIES CXX_SCAN_FOR_MODULES OFF)
  add_executable(module-test "${t}/module-test.cc")
  target_include_directories(module-test PRIVATE "${t}")
  target_link_libraries(module-test PRIVATE kiln-fmt-test-main fmt)
  add_test(NAME module-test COMMAND module-test)
  add_executable(fmt-module-smoke "${CMAKE_CURRENT_FUNCTION_LIST_DIR}/fmt-module-smoke.cc")
  target_link_libraries(fmt-module-smoke PRIVATE fmt)
  add_test(NAME fmt-module-smoke COMMAND fmt-module-smoke)
  add_custom_target(fmt_module_tests DEPENDS module-test fmt-module-smoke)
endfunction()

cmake_language(DEFER DIRECTORY "${CMAKE_SOURCE_DIR}" CALL _kiln_fmt_module_tests)
