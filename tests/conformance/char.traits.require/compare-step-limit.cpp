// [implimits], [char.traits.require]: a bulk compare that would cross the
// step limit declines, so the limit fires inside the library's loop, with
// the call's notes (spec/oven.md "Library fast paths").
//@ args: -fconstexpr-steps=5
//@ expect: compile-fail
//@ diag: 14:15 error "static assertion expression is not an integral constant expression"
//@ diag: 12:10 note "in call to 'compare(&a[0], &a[0], 10)'"
//@ diag: 14:15 note "in call to 'same()'"
#include <string>
constexpr int same() {
  const char a[] = "aaaaaaaaaa";
  return std::char_traits<char>::compare(a, a, 10);
}
static_assert(same() == 0);
