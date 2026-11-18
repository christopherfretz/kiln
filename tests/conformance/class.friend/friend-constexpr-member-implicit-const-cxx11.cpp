// [dcl.constexpr]/8 (C++11): a constexpr non-static member function is
// implicitly const, a friend declaration's included, so `friend constexpr
// int C::k();` names a const member and does not match a non-const `int
// k()`; it does match a const one, which it then redeclares with a
// different constexpr specifier. kiln skipped the implicit const for a
// friend. Same diagnostics as clang 22 (GCC 16.1 rejects `k` too and does
// not compare a friend's constexpr specifier).
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ args: -std=c++11
//@ diag: 24:27 warning "'constexpr' non-static member function will not be implicitly 'const' in C++14; add 'const' to avoid a change in behavior [-Wconstexpr-not-const]"
//@ diag: 24:27 error "friend declaration of 'k' does not match any declaration in 'C'"
//@ diag: 20:7 note "candidate function"
//@ diag: 25:27 warning "'constexpr' non-static member function will not be implicitly 'const' in C++14; add 'const' to avoid a change in behavior [-Wconstexpr-not-const]"
//@ diag: 25:27 error "constexpr declaration of 'm' follows non-constexpr declaration"
//@ diag: 21:7 note "previous declaration is here"

struct C {
  int k();
  int m() const;
};
class D {
  friend constexpr int C::k();
  friend constexpr int C::m();
};
