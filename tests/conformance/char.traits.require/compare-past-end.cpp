// [char.traits.require], [expr.const]/5.8: char_traits<char>::compare
// reading past the end of an array is no constant expression. The Oven's
// bulk compare (spec/oven.md "Library fast paths") declines and the
// library's loop runs, so the notes are the call's.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:15 error "static assertion expression is not an integral constant expression"
//@ diag: 14:10 note "in call to 'compare(&a[0], &b[0], 5)'"
//@ diag: 16:15 note "in call to 'past_end()'"
#include <string>
constexpr int past_end() {
  const char a[3] = {'a', 'b', 'c'};
  const char b[5] = {'a', 'b', 'c', 'd', 'e'};
  return std::char_traits<char>::compare(a, b, 5);
}
static_assert(past_end() < 0);
