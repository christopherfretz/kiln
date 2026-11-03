//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:3 error "inline declaration of 'x' not allowed in block scope"
//@ diag: 19:10 error "inline declaration of 'y' not allowed in block scope"
//@ diag: 20:3 error "declaration cannot be declared 'inline'"
//@ diag: 20:3 error "inline declaration of 'a' not allowed in block scope"
//@ diag: 21:17 error "inline declaration of 'z' not allowed in block scope"
// [dcl.inline]: the inline specifier shall not appear on a block scope
// declaration (clang `err_inline_declaration_block_scope`, at `inline`):
// a local variable, a static local, a lambda body's local and a
// structured binding declaration, named by its first binding. A
// namespace-scope inline variable is fine. clang 19 says "decomposition
// declaration", clang 22 "structured binding declaration".
struct P { int a, b; };
inline int g = 1;
int f() {
  inline int x = 1;
  static inline const int y = 2;
  inline auto [a, b] = P{1, 2};
  auto l = [] { inline int z = 3; return z; };
  return x + y + a + b + l();
}
