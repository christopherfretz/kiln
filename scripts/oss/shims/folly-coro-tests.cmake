# folly_coro (WP-789): included after `project(folly)` through
# -DCMAKE_PROJECT_folly_INCLUDE, in place of folly-no-liburing.cmake (which
# it includes, so the same liburing workaround applies). folly's CMake build
# never defines the folly/coro tests: they exist only in its BUCK files. This
# defers, to the end of folly's top-level CMakeLists.txt (where
# folly_define_tests, folly_test_support and HAVE_CMAKE_GTEST exist when
# BUILD_TESTS=ON), one gtest executable per folly/coro/test/*Test.cpp and
# folly/coro/safe/test/*Test.cpp, named coro_<file> / coro_safe_<file>, plus
# the aggregate target `folly_coro_tests` that builds exactly them (and so
# libfolly and folly_test_support). Their CTest names start with `coro_`.
include("${CMAKE_CURRENT_LIST_DIR}/folly-no-liburing.cmake")

function(_kiln_folly_coro_tests)
  if(NOT TARGET folly_test_support)
    message(FATAL_ERROR "folly-coro-tests.cmake needs -DBUILD_TESTS=ON")
  endif()
  set(all)
  foreach(sub "coro/test" "coro/safe/test")
    file(GLOB srcs RELATIVE "${FOLLY_DIR}/${sub}" "${FOLLY_DIR}/${sub}/*Test.cpp")
    list(SORT srcs)
    string(REPLACE "/" "_" prefix "${sub}")
    string(REPLACE "_test" "" prefix "${prefix}")
    set(args DIRECTORY "${sub}/")
    foreach(src IN LISTS srcs)
      string(REGEX REPLACE "\\.cpp$" "" stem "${src}")
      set(name "${prefix}_${stem}")
      list(APPEND args TEST "${name}" SOURCES "${src}")
      list(APPEND all "${name}")
    endforeach()
    folly_define_tests(${args})
  endforeach()
  add_custom_target(folly_coro_tests DEPENDS ${all})
endfunction()

cmake_language(DEFER DIRECTORY "${CMAKE_SOURCE_DIR}" CALL _kiln_folly_coro_tests)
