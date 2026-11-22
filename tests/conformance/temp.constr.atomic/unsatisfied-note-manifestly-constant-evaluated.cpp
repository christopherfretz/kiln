// [temp.constr.atomic]/3, [expr.const]/22: an atomic constraint is a
// constant expression, so manifestly constant-evaluated; the notes of an
// unsatisfied one that print operand values ("'f() == 5' (1001 == 5)") and
// pick the false operands of a `&&` left in a substituted atom evaluate
// them so too: `if consteval` takes its compound statement (clang 19 and
// 22 alike; WP-1943).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 43:3 error "no matching function for call to 'h'"
//@ diag: 36:43 note "candidate template ignored: constraints not satisfied [with N = 5]"
//@ diag: 36:28 note "because 'f() == 5' (1001 == 5) evaluated to false"
//@ diag: 44:3 error "no matching function for call to 'k'"
//@ diag: 37:46 note "candidate template ignored: constraints not satisfied [with N = 5]"
//@ diag: 37:28 note "because 'g() + 5 < 0' (2007 < 0) evaluated to false"
//@ diag: 45:13 error "no matching member function for call to 'm'"
//@ diag: 39:55 note "candidate template ignored: constraints not satisfied [with T = int]"
//@ diag: 39:33 note "because 't<1>()' evaluated to false"
#include <type_traits>

constexpr int f() {
  if consteval {
    return 1001;
  } else {
    return 1;
  }
}
constexpr int g() { return std::is_constant_evaluated() ? 2002 : 2; }
template <int N> constexpr bool t() {
  if consteval {
    return N != 1;
  } else {
    return N != 2;
  }
}

template <int N> requires (f() == N) void h();
template <int N> requires (g() + N < 0) void k();
template <int... Ns> struct S {
  template <class T> requires ((t<Ns>() && ...)) void m();
};

void use() {
  h<5>();
  k<5>();
  S<1, 2>{}.m<int>();
}
