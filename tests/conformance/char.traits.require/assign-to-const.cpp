// [char.traits.require], [expr.const]/10: char_traits<char>::assign to a
// const object is no constant expression. The Oven's native assign
// (spec/oven.md "Library fast paths") declines and the library's code
// runs, so the notes are the call's. Clang 19 accepts the write.
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:15 error "static assertion expression is not an integral constant expression"
//@ diag: 14:3 note "in call to 'assign(a, 'b')'"
//@ diag: 17:15 note "in call to 'to_const()'"
#include <string>
constexpr char to_const() {
  const char a = 'a';
  std::char_traits<char>::assign(const_cast<char &>(a), 'b');
  return a;
}
static_assert(to_const() == 'b');
