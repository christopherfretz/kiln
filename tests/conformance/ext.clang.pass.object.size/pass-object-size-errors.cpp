// clang extension `pass_object_size` / `overloadable` (WP-803): the attribute takes one integer
// constant in 0..=3 (reported at the argument), applies only to pointer parameters (a const
// pointer in a definition, reported at the parameter's name), is ignored with a warning on a
// non-parameter; `overloadable` on a non-function is an error. A pass_object_size overload of
// a declaration without it is a distinct function, not a redeclaration.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:9 error "'pass_object_size' attribute only applies to constant pointer arguments"
//@ diag: 19:47 error "'pass_object_size' attribute requires integer constant between 0 and 3 inclusive"
//@ diag: 20:30 error "'pass_object_size' attribute takes one argument"
//@ diag: 21:22 warning "'pass_object_size' attribute only applies to parameters [-Wignored-attributes]"
//@ diag: 22:22 error "'overloadable' attribute only applies to functions"
//@ diag: 28:52 error "pass_object_size attribute only applies to constant pointer arguments"

// Line numbers above are pinned: keep the declarations below on lines 18-29.
// (non-pointer, out of range, missing argument, variable, overloadable variable)
// ---
void f1(int __attribute__((pass_object_size(0))) x);
void f2(char *__attribute__((pass_object_size(4))) p);
void f3(char *__attribute__((pass_object_size)) p);
int v __attribute__((pass_object_size(0)));
int w __attribute__((overloadable));
void f4(char *const __attribute__((pass_object_size(0))) p);
void f4(char *p);
void g() { char b[4]; f4(b); }
// A non-const pointer is accepted on a declaration, not on a definition.
void f5(char *__attribute__((pass_object_size(0))) p);
void f7(char *__attribute__((pass_object_size(0))) p) {}
void f8(char *const __attribute__((pass_object_size(0))) p) {}
