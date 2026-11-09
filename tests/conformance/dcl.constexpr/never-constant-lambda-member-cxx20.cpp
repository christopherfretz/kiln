// [dcl.constexpr]/6 (C++20, no diagnostic required): clang's never-constant
// check (`Expr::isPotentialConstantExpr`, an error by default before
// C++23) covers the call operator of a lambda declared `constexpr` and a
// non-static member function: control flowing through the definition of a
// static or thread_local variable not usable in constant expressions, or
// reaching the end of a function returning non-`void`, never yields a
// constant (WP-1123). The C++23 extension warning for a static variable
// in a constexpr function is off here.
//@ expect: compile-fail
//@ std: c++20
//@ args: -Wno-c++23-extensions
//@ strict-diags
//@ diag: 23:25 error "constexpr function never produces a constant expression"
//@ diag: 23:53 note "control flows through the definition of a static variable"
//@ diag: 24:31 error "constexpr function never produces a constant expression"
//@ diag: 25:20 note "control flows through the definition of a thread_local variable"
//@ diag: 29:17 error "constexpr function never produces a constant expression"
//@ diag: 29:45 note "control flows through the definition of a static variable"
//@ diag: 30:17 error "constexpr function never produces a constant expression"
//@ diag: 30:62 note "control flows through the definition of a static variable"
//@ diag: 32:18 error "no return statement in constexpr function"

auto static_in_lambda = []() constexpr { static int i = 0; return i; };
auto thread_local_in_lambda = [](int x) constexpr {
  thread_local int i = 0;
  return i + x;
};
struct S {
  constexpr int member() const { static int i = 0; return i; }
  constexpr int after_read(int x) const { x++; static double d = 1; return x; }
};
auto no_return = []() constexpr -> int {};

// Valid: a `const` integral static usable in constant expressions, a
// `static constexpr` one, a lambda deducing `void`.
auto usable = []() constexpr { static const int i = 0; return i; };
auto usable_constexpr = []() constexpr { static constexpr int i = 1; return i; };
auto void_lambda = []() constexpr {};
