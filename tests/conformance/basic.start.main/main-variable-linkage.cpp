// [basic.start.main]/3: a global variable `main` declared in a C++
// linkage specification is diagnosed at its decl-specifiers (clang's
// err_main_global_variable; WP-1015; clang 22 rewords it, so the message
// is matched by prefix).
//@ std: c++26
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:14 error "main cannot be declared as "
extern "C++" int main;
