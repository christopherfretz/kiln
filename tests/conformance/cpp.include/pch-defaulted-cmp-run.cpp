// [class.compare.default]/1-6, [class.eq]/2-3, [except.spec]/9: a
// header's defaulted comparison operators (member and friend, in a class
// and a class template; libc++'s `partial_ordering`, `to_chars_result`)
// compare the subobjects with the operators the `= default` found, and
// their implicit exception specifications follow those operators. With
// `--pch-diff`, the header's snapshot carries them with their
// `default` keywords and captured lookups (WP-1314).
//@ expect: run
#include "Inputs/defaulted_cmp.h"
static_assert(noexcept(dc::tc{} == dc::tc{}) && !noexcept(dc2::U{} == dc2::U{}));
bool g() { return dc2::U{{1}} == dc2::U{{1}}; }
static_assert(dc::po{2} == dc::po{2} && dc::po{2} != dc::po{3});
static_assert(dc::box<int>{4} == dc::box<int>{4});
static_assert(noexcept(dc::po{} == dc::po{}));
int main() {
  dc::tc a{1, 2}, b{1, 3};
  dc::mm m{1}, n{1};
  dc::box<long> x{5}, y{6};
  return !g() + (a == b) + (m != n) + (x == y) + !dc::used;
}
