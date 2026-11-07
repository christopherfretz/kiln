# folly (WP-726): included after `project(folly)` through
# -DCMAKE_PROJECT_folly_INCLUDE. folly enables its io_uring code whenever
# <liburing.h> exists (folly/io/async/Liburing.h tests `defined(__linux__) &&
# __has_include(<liburing.h>)`, not the CMake LibUring result), and that code
# needs liburing >= 2.10 (zero-copy receive, io_uring_set_iowait), which the
# host's liburing 2.9 lacks: those TUs fail with every compiler, the
# monolithic libfolly never links and no test builds. Put a replacement
# folly/io/async/Liburing.h that says FOLLY_HAS_LIBURING 0 first on every
# include path, so folly builds as on a host without liburing; the same for
# kiln and the clang control.
include_directories(BEFORE "${CMAKE_CURRENT_LIST_DIR}/folly-include")
