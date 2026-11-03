//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ args: -ferror-limit=0
//@ diag: 48:1 error "structured binding declaration cannot be declared 'inline'"
//@ diag: 48:8 error "structured binding declaration cannot be declared 'constexpr'"
//@ diag: 49:1 error "structured binding declaration cannot be declared 'typedef'"
//@ diag: 49:9 error "structured binding declaration cannot be declared 'inline'"
//@ diag: 51:3 error "structured binding declaration cannot be declared 'inline'"
//@ diag: 51:3 error "inline declaration of 'a1' not allowed in block scope"
//@ diag: 52:10 error "structured binding declaration cannot be declared 'inline'"
//@ diag: 52:10 error "inline declaration of 'a2' not allowed in block scope"
//@ diag: 53:3 error "structured binding declaration cannot be declared 'inline'"
//@ diag: 53:3 error "inline declaration of 'a3' not allowed in block scope"
//@ diag: 54:13 error "structured binding declaration cannot be declared 'inline'"
//@ diag: 54:3 error "structured binding declaration cannot be declared 'constexpr'"
//@ diag: 54:13 error "inline declaration of 'a4' not allowed in block scope"
//@ diag: 55:3 error "structured binding declaration cannot be declared 'extern'"
//@ diag: 55:10 error "structured binding declaration cannot be declared 'inline'"
//@ diag: 55:10 error "inline declaration of 'a5' not allowed in block scope"
//@ diag: 55:22 error "declaration of block scope identifier with linkage cannot have an initializer"
//@ diag: 56:3 error "structured binding declaration cannot be declared 'register'"
//@ diag: 56:12 error "structured binding declaration cannot be declared 'constexpr'"
//@ diag: 56:3 error "ISO C++17 does not allow 'register' storage class specifier"
//@ diag: 57:3 error "structured binding declaration cannot be declared 'inline'"
//@ diag: 57:10 error "structured binding declaration cannot be declared 'constinit'"
//@ diag: 57:3 error "inline declaration of 'a7' not allowed in block scope"
//@ diag: 57:10 error "local variable cannot be declared 'constinit'"
//@ diag: 58:3 error "structured binding declaration cannot be declared 'extern'"
//@ diag: 58:10 error "structured binding declaration cannot be declared 'constinit'"
//@ diag: 58:25 error "declaration of block scope identifier with linkage cannot have an initializer"
//@ diag: 59:3 error "structured binding declaration cannot be declared 'register'"
//@ diag: 59:12 error "structured binding declaration cannot be declared 'inline'"
//@ diag: 59:3 error "ISO C++17 does not allow 'register' storage class specifier"
//@ diag: 59:12 error "inline declaration of 'a9' not allowed in block scope"
// [dcl.pre]/6 (C++20): a structured binding declaration's
// decl-specifier-seq holds only static, thread_local, auto and
// cv-qualifiers; [dcl.inline]: `inline` shall not appear on a block scope
// declaration. clang 22 reports each invalid specifier with its own
// error at its location (storage class, then `inline`, then the
// constexpr kind), and a block-scope `inline` also with
// "inline declaration of 'a1' not allowed in block scope" naming the
// first binding (clang 19: one error naming them all,
// specifiers-each-19.cpp).
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
