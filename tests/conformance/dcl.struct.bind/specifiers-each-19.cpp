//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang<22
//@ args: -ferror-limit=0
//@ diag: 36:8 error "decomposition declaration cannot be declared with 'constexpr inline' specifiers"
//@ diag: 37:1 error "decomposition declaration cannot be declared with 'typedef inline' specifiers"
//@ diag: 39:3 error "decomposition declaration cannot be declared 'inline'"
//@ diag: 39:3 error "inline declaration of 'a1' not allowed in block scope"
//@ diag: 40:10 error "decomposition declaration cannot be declared 'inline'"
//@ diag: 40:10 error "inline declaration of 'a2' not allowed in block scope"
//@ diag: 41:3 error "decomposition declaration cannot be declared 'inline'"
//@ diag: 41:3 error "inline declaration of 'a3' not allowed in block scope"
//@ diag: 42:3 error "decomposition declaration cannot be declared with 'constexpr inline' specifiers"
//@ diag: 42:13 error "inline declaration of 'a4' not allowed in block scope"
//@ diag: 43:3 error "decomposition declaration cannot be declared with 'extern inline' specifiers"
//@ diag: 43:10 error "inline declaration of 'a5' not allowed in block scope"
//@ diag: 43:22 error "declaration of block scope identifier with linkage cannot have an initializer"
//@ diag: 44:3 error "decomposition declaration cannot be declared with 'register constexpr' specifiers"
//@ diag: 44:3 error "ISO C++17 does not allow 'register' storage class specifier"
//@ diag: 45:10 error "decomposition declaration cannot be declared with 'constinit inline' specifiers"
//@ diag: 45:3 error "inline declaration of 'a7' not allowed in block scope"
//@ diag: 45:10 error "local variable cannot be declared 'constinit'"
//@ diag: 46:3 error "decomposition declaration cannot be declared with 'extern constinit' specifiers"
//@ diag: 46:25 error "declaration of block scope identifier with linkage cannot have an initializer"
//@ diag: 47:3 error "decomposition declaration cannot be declared with 'register inline' specifiers"
//@ diag: 47:3 error "ISO C++17 does not allow 'register' storage class specifier"
//@ diag: 47:12 error "inline declaration of 'a9' not allowed in block scope"
// [dcl.pre]/6 (C++20), [dcl.inline], at -fclang-compat=19: clang 19
// reports the invalid specifiers of a structured binding declaration as
// one error at the first, naming them all, and a block-scope `inline`
// also with "inline declaration of 'a1' not allowed in block scope"
// (clang 22: one error per specifier, specifiers-each.cpp).
struct P { int a, b; };
P p;
inline constexpr auto [n3, n4] = P{1, 2};
typedef inline auto [t1, t2] = p;
void f() {
  inline auto [a1, b1] = p;
  static inline auto [a2, b2] = p;
  inline static thread_local auto [a3, b3] = p;
  constexpr inline auto [a4, b4] = P{1, 2};
  extern inline auto [a5, b5] = p;
  register constexpr auto [a6, b6] = P{1, 2};
  inline constinit auto [a7, b7] = p;
  extern constinit auto [a8, b8] = p;
  register inline const auto [a9, b9] = p;
}
