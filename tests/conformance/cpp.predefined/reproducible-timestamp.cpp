// __TIMESTAMP__ (the GNU extension beside [cpp.predefined]/1's __DATE__ and
// __TIME__) under -fkiln-reproducible-time is the fixed epoch's spelling,
// "Thu Jan  1 00:00:00 1970", in every file whatever its modification time
// (WP-2323, spec/driver.md "Reproducible time").
//@ expect: compile-ok
//@ strict-diags
//@ args: -fkiln-reproducible-time
//@ skip(clang): -fkiln-reproducible-time is kiln's flag (clang pins only through SOURCE_DATE_EPOCH)
//@ skip(gcc): -fkiln-reproducible-time is kiln's flag
#include "Inputs/timestamp.h"

constexpr bool same(const char *a, const char *b) {
  for (; *a && *a == *b; ++a, ++b) {
  }
  return *a == *b;
}

static_assert(same(__TIMESTAMP__, "Thu Jan  1 00:00:00 1970"));
static_assert(same(header_stamp, __TIMESTAMP__));
static_assert(same(__DATE__ " " __TIME__, "Jan  1 1970 00:00:00"));
