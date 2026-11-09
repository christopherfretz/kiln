// [dcl.constexpr] (C++23, P2448R2): under `-Winvalid-constexpr` clang still
// checks that some call could be constant (`Expr::isPotentialConstantExpr`):
// reaching the end of a function returning non-`void` ("control reached
// end") or the definition of a static or thread_local variable not usable
// in constant expressions fails, for a lambda declared `constexpr`, a
// non-static member and a function with parameters alike (WP-1123).
//@ expect: compile-fail
//@ std: c++23
//@ args: -Winvalid-constexpr -Wno-return-type
//@ strict-diags
//@ diag: 30:18 error "constexpr function never produces a constant expression"
//@ diag: 30:41 note "control reached end of constexpr function"
//@ diag: 32:17 error "constexpr function never produces a constant expression"
//@ diag: 32:33 note "control reached end of constexpr function"
//@ diag: 33:17 error "constexpr function never produces a constant expression"
//@ diag: 33:47 note "control reached end of constexpr function"
//@ diag: 35:15 error "constexpr function never produces a constant expression"
//@ diag: 35:43 note "control reached end of constexpr function"
//@ diag: 36:15 error "constexpr function never produces a constant expression"
//@ diag: 36:46 note "control reached end of constexpr function"
//@ diag: 37:15 error "constexpr function never produces a constant expression"
//@ diag: 37:57 note "control reached end of constexpr function"
//@ diag: 38:15 error "constexpr function never produces a constant expression"
//@ diag: 38:58 note "control flows through the definition of a thread_local variable"
//@ diag: 39:15 error "constexpr function never produces a constant expression"
//@ diag: 39:51 note "control flows through the definition of a static variable"
//@ diag: 40:15 error "constexpr function never produces a constant expression"
//@ diag: 40:66 note "control flows through the definition of a static variable"

auto no_return = []() constexpr -> int {};
struct S {
  constexpr int member() const {}
  constexpr int local(int) const { int y = 1; }
};
constexpr int discarded(int x) { (void)x; }
constexpr int known_if(int x) { if (true) {} }
constexpr int local_class(int x) { struct L { int a; }; }
constexpr int thread_local_var(int x) { thread_local int i = 0; return i + x; }
constexpr int after_read(int x) { x++; static int i = 0; return 0; }
constexpr int after_unknown_local(int x) { int y = x; static int i = 0; return y; }

// Valid (the check does not see a failure): an unknown local is
// destroyed quietly at the end of its block, an assumption naming a
// parameter, a try block, an unknown condition, a `static constexpr` and
// a usable `const` static, a block-scope `extern` declaration.
constexpr int unknown_local(int x) { int y = x; }
constexpr int assumption(int x) { [[assume(x > 0)]]; }
constexpr int try_block(int x) { try { return x; } catch (...) {} }
constexpr int unknown_if(int x) { if (x) return 1; }
constexpr int usable(int x) { static constexpr int a = 1; static const int b = 2; return a + b + x; }
constexpr int ext(int x) { extern const int e; return x; }
