// [basic.start.main]/3: `main` shall not be declared static or inline
// (clang's err_static_main / err_inline_main, at the specifier).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:1 error "'main' is not allowed to be declared static"
//@ diag: 7:8 error "'main' is not allowed to be declared inline"
static inline int main() { return 0; }
