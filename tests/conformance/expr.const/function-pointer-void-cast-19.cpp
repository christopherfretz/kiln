// [expr.cast]/4, [expr.const]/10.15: clang 19 folds a C-style cast
// between a pointer to function and `void *` in a constant expression
// (a known clang 19 accepts-invalid); kiln at -fclang-compat=19 follows
// it. clang 22 and GCC 16.1 reject: function-pointer-void-cast.cpp (WP-830).
//@ expect: compile-ok
//@ requires: clang<22
void f() {}
constexpr const void *p = (const void *)&f;
constexpr bool b = (const void *)&f != nullptr;
static_assert((const void *)&f == (const void *)&f);
