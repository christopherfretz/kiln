// [implimits]: a std::min call one past the depth limit exceeds it, as
// the library's call does. The Oven's native std::min (spec/oven.md
// "Library fast paths") declines within three calls of the limit, so the
// limit fires at the call with the library's notes.
//@ args: -fconstexpr-depth=9
//@ expect: compile-fail
//@ diag: 12:15 error "static assertion expression is not an integral constant expression"
//@ diag: 11:45 note "constexpr evaluation exceeded maximum depth of 9 calls"
//@ diag: 11:62 note "in call to 'down(0)'"
#include <algorithm>
constexpr int down(int n) { return n == 0 ? std::min(n, 1) : down(n - 1); }
static_assert(down(8) == 0);
