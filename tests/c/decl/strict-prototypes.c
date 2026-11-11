// C17 6.11.6: a function declarator with empty parentheses is an
// obsolescent feature; clang's -Wstrict-prototypes (in -pedantic, not in
// -Wall or -Wextra) at every such declarator's `(`, a K&R definition
// getting -Wdeprecated-non-prototype instead (WP-1312, WP-1299 follow-up).
//@ std: c17
//@ args: -Wstrict-prototypes
//@ expect: compile-ok
//@ strict-diags
//@ diag: 15:7 warning "a function declaration without a prototype is deprecated in all versions of C"
//@ diag: 16:7 warning "a function declaration without a prototype is deprecated in all versions of C"
//@ diag: 17:5 warning "a function definition without a prototype is deprecated in all versions of C and is not supported in C23"
//@ diag: 18:11 warning "a function declaration without a prototype is deprecated in all versions of C"
//@ diag: 19:17 warning "a function declaration without a prototype is deprecated in all versions of C"
//@ diag: 20:16 warning "a function declaration without a prototype is deprecated in all versions of C"
void f();
void g() {}
int h(a) int a; { return a; }
void (*fp)();
typedef int fn_t();
int k(int (*cb)());
void ok(void);
