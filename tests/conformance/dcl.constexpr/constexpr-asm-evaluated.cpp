// [dcl.constexpr]/3, [expr.const]/5: since C++20 an asm-declaration may
// appear in a constexpr function or constructor body; it is valid as long
// as constant evaluation does not reach it, and evaluating it is not a
// constant expression (WP-890, WP-891). Verified against clang 19 and 22.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:15 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 17:5 note "subexpression not valid in a constant expression"
//@ diag: 21:19 note "in call to 'f(-1)'"
//@ diag: 31:13 error "constexpr variable 's2' must be initialized by a constant expression"
//@ diag: 26:7 note "subexpression not valid in a constant expression"
//@ diag: 31:13 note "in call to 'S(-1)'"

constexpr int f(int x) {
  if (x < 0)
    asm("");
  return x;
}
static_assert(f(1) == 1);
constexpr int a = f(-1);

struct S {
  constexpr S(int x) : v(x) {
    if (x < 0)
      asm("");
  }
  int v;
};
constexpr S s1(1);
constexpr S s2(-1);
