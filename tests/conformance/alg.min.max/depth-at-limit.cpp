// [implimits]: a std::min call that reaches the depth limit fits, as the
// library's call does: the Oven's native std::min (spec/oven.md "Library
// fast paths") declines within three calls of the limit and the library
// code runs (depth-limit.cpp is one call deeper).
//@ args: -fconstexpr-depth=10
//@ expect: compile-ok
#include <algorithm>
constexpr int down(int n) { return n == 0 ? std::min(n, 1) : down(n - 1); }
static_assert(down(8) == 0);
