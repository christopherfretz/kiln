// __builtin_types_compatible_p is a C-only GNU builtin: in C++ clang has
// no such builtin, and the name is an ordinary identifier.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:41 error "expected '(' for function-style cast or type construction"
//@ diag: 10:46 error "expected '(' for function-style cast or type construction"
#if __has_builtin(__builtin_types_compatible_p)
#error
#endif
int a = __builtin_types_compatible_p(int, int);
