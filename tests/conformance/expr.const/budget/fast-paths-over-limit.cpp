// [implimits], [expr.const]/5.1: the library fast paths (a wrapper of the
// evaluation context costs no step, a char_traits bulk operation one per
// element; spec/oven.md "Library fast paths") still leave the step limit
// firing on a loop of such calls: the loop's own statements count.
//@ expect: compile-fail
//@ diag: 20:15 error "static assertion expression is not an integral constant expression"
//@ diag: 14:16 note "constexpr evaluation hit maximum step limit; possible infinite loop?"
//@ diag: 20:15 note "in call to 'spin()'"
#include <string>
#include <type_traits>
constexpr bool spin() {
  const char a[] = "abc";
  int n = 0;
  while (true) {
    n += std::char_traits<char>::compare(a, a, 3);
    n += std::is_constant_evaluated();
  }
  return n;
}
static_assert(spin());
