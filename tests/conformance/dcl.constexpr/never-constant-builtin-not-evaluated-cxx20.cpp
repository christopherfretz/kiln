// [dcl.constexpr]/6, [expr.const]/10: a call of a builtin clang does not
// constant-evaluate (no `Constexpr` marker in Builtins.td, so
// `__has_constexpr_builtin` is 0) fails at its callee before its
// arguments, even where kiln's Oven folds it as GCC does (`__builtin_floor`,
// P0533): clang 19 reports the function as never constant wherever such a
// call is on the known path, whatever else is unknown (WP-990, WP-945).
// spec/sema.md "Never constant".
//@ std: c++20
//@ strict-diags
//@ expect: compile-fail
//@ args: -Wno-unused-value
//@ diag: 36:18 error "constexpr function never produces a constant expression"
//@ diag: 36:44 note "subexpression not valid in a constant expression"
//@ diag: 37:18 error "constexpr function never produces a constant expression"
//@ diag: 37:40 note "subexpression not valid in a constant expression"
//@ diag: 38:15 error "constexpr function never produces a constant expression"
//@ diag: 38:46 note "subexpression not valid in a constant expression"
//@ diag: 39:18 error "constexpr function never produces a constant expression"
//@ diag: 39:32 note "subexpression not valid in a constant expression"
//@ diag: 40:15 error "constexpr function never produces a constant expression"
//@ diag: 40:27 note "subexpression not valid in a constant expression"
//@ diag: 41:18 error "constexpr function never produces a constant expression"
//@ diag: 41:41 note "subexpression not valid in a constant expression"
//@ diag: 42:15 error "constexpr function never produces a constant expression"
//@ diag: 42:34 note "subexpression not valid in a constant expression"
//@ diag: 43:15 error "constexpr function never produces a constant expression"
//@ diag: 43:35 note "subexpression not valid in a constant expression"
//@ diag: 44:18 error "constexpr function never produces a constant expression"
//@ diag: 44:40 note "subexpression not valid in a constant expression"
//@ diag: 47:13 error "constexpr constructor never produces a constant expression"
//@ diag: 47:30 note "subexpression not valid in a constant expression"
//@ diag: 48:20 error "constexpr function never produces a constant expression"
//@ diag: 48:43 note "subexpression not valid in a constant expression"
//@ diag: 50:15 error "constexpr function never produces a constant expression"
//@ diag: 52:20 note "subexpression not valid in a constant expression"
constexpr double f1(double x) { return x + __builtin_floor(1.0); }
constexpr double f2(double x) { return __builtin_floor(2.5) + x; }
constexpr int f3(const int *p) { return (int)__builtin_floor(*p); }
constexpr double f4() { return __builtin_floor(2.5); }
constexpr int f5(int x) { __builtin_trap(); return x; }
constexpr double f6(int x) { return (x, __builtin_floor(1.0)); }
constexpr int f7(int x) { return __builtin_floor(1.0) ? x : x; }
constexpr int f8(int x) { int y = __builtin_floor(1.0); return x + y; }
constexpr double f9(double x) { return (__builtin_floor)(1.0) + x; }
struct S {
  int v;
  constexpr S(int x) : v(int(__builtin_floor(1.0)) + x) {}
  constexpr double m() const { return v + __builtin_sqrt(4.0); }
};
constexpr int f10(int x) {
  if (true)
    return x + int(__builtin_ceil(0.5));
  return 0;
}
