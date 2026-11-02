// GNU statement expressions under clang 19: trailing null statements are
// skipped when finding the value, so `({ e; ; })` has e's type. clang 22 made
// that `void` (trailing-null.cpp); kiln follows -fclang-compat.
//@ expect: compile-ok
//@ requires: clang<22
//@ strict-diags
//@ diag: 11:3 warning "expression result unused"
void f() {
  static_assert(__is_same(decltype(({ 1; ; })), int));
  static_assert(__is_same(decltype(({ 1; })), int));
  ({ 1; ; });
}
