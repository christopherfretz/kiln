// WP-1331, C23 6.4.4.1p3: a `wb` literal is `_BitInt(N)` with the smallest
// N holding the value and a sign bit (at least 2), a `uwb` literal
// `unsigned _BitInt(N)` with the smallest N holding it (at least 1); `wb`
// and `WB` (not `wB`/`Wb`) combine with `u`/`U` in either order. Types
// proven by _Generic, sizes by sizeof, as clang 19/22.
//@ std: c23
//@ args: -Werror
//@ expect: run
//@ stdout: s2 u1 s2 u1 s9 u8 s8 u1 u1
//@ stdout: u64 s65 u128 s128 u3
//@ stdout: 1 2 16 16
//@ stdout: 1 0 6 1
#include <stdio.h>

#define T(x)                                                            \
  _Generic((x), _BitInt(2): "s2", unsigned _BitInt(1): "u1",            \
           _BitInt(9): "s9", unsigned _BitInt(8): "u8", _BitInt(8): "s8", \
           unsigned _BitInt(64): "u64", _BitInt(65): "s65",             \
           unsigned _BitInt(128): "u128", _BitInt(128): "s128",         \
           unsigned _BitInt(3): "u3", default: "other")

int main(void) {
  printf("%s %s %s %s %s %s %s %s %s\n", T(0wb), T(0uwb), T(1wb), T(1Uwb),
         T(255wb), T(255uwb), T(0x7fWB), T(1wbU), T(1WBu));
  printf("%s %s %s %s %s\n", T(18446744073709551615uwb),
         T(18446744073709551615wb), T(0xffffffffffffffffffffffffffffffffuwb),
         T(0x7fffffffffffffffffffffffffffffffwb), T(0b101uwb));
  printf("%zu %zu %zu %zu\n", sizeof(0wb), sizeof(255wb),
         sizeof(18446744073709551615wb), sizeof(0x7fffffffffffffffffffffffffffffffwb));
  printf("%d %d %d %d\n", (int)(-1wb < 0), (int)(1uwb - 2 > 0),
         (int)(0b101wb + 0'1wb),
         (int)(0xffffffffffffffffffffffffffffffffuwb >> 127));
  return 0;
}
