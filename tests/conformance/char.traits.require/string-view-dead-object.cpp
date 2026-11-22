// [basic.life]/7, [expr.const]/5.8: comparing a string_view whose string
// has been destroyed reads an object outside its lifetime. The bulk
// compare declines and the library's loop diagnoses the read (spec/oven.md
// "Library fast paths").
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:15 error "static assertion expression is not an integral constant expression"
//@ diag: 18:10 note "in call to 'operator==<char, "
//@ diag: 20:15 note "in call to 'dead_view()'"
#include <string>
#include <string_view>
constexpr bool dead_view() {
  std::string_view v;
  {
    std::string s = "abc";
    v = s;
  }
  return v == "abc";
}
static_assert(dead_view());
