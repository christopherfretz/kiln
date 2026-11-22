// [char.traits.require], [expr.const]/10: char_traits<char>::assign from
// an uninitialized character is no constant expression. The Oven's native
// assign (spec/oven.md "Library fast paths") declines and the library's
// code runs, so the notes are the call's. assign-to-const.cpp has the
// write to a const object.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:15 error "static assertion expression is not an integral constant expression"
//@ diag: 15:3 note "in call to 'assign(b, a)'"
//@ diag: 18:15 note "in call to 'from_uninit()'"
#include <string>
constexpr char from_uninit() {
  char a;
  char b = 'b';
  std::char_traits<char>::assign(b, a);
  return b;
}
static_assert(from_uninit() == 'b');
