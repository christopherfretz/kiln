// GNU statement expressions: the value comes from the LAST statement, and a
// trailing null statement is a statement, so `({ e; ; })` is `void` (clang 22,
// matching GCC's "the last thing in the compound statement"). clang 19 skipped
// trailing null statements (trailing-null-19.cpp); kiln follows -fclang-compat.
//@ expect: compile-ok
//@ requires: clang>=22
//@ strict-diags
//@ diag: 12:6 warning "expression result unused"
void f() {
  static_assert(__is_same(decltype(({ 1; ; })), void));
  static_assert(__is_same(decltype(({ 1; })), int));
  ({ 1; ; });
}
