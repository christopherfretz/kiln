// folly (WP-726): replaces folly/io/async/Liburing.h through
// scripts/oss/shims/folly-no-liburing.cmake; the host's liburing is too old
// for folly's io_uring code, so build as if it were absent.
#pragma once

#define FOLLY_HAS_LIBURING 0
