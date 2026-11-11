// C23 6.2.1p4 (scopes of identifiers): a tag first declared in a parameter
// list has function prototype scope, or the block scope of the function
// body for a definition, so nothing outside the function can name it
// (clang -Wvisibility). C, unlike C++ [dcl.fct]/17, also lets a tag be
// defined in a return type or a sizeof operand (C23 6.7.2.3).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:15 warning "declaration of 'struct P' will not be visible outside of this function"
//@ diag: 17:13 warning "declaration of 'enum G' will not be visible outside of this function"
//@ diag: 18:14 warning "declaration of 'union Q' will not be visible outside of this function"
//@ diag: 21:15 warning "declaration of 'struct K' will not be visible outside of this function"
//@ diag: 22:15 warning "declaration of 'struct N' will not be visible outside of this function"
//@ diag: 23:14 warning "declaration of 'struct O' will not be visible outside of this function"
//@ diag: 26:22 error "invalid application of 'sizeof' to an incomplete type 'struct O'"
//@ diag: 26:36 note "forward declaration of 'struct O'"
void f(struct P { int x; } p);
void g(enum G { A, B } e);
void h(union Q *q);
struct K { int a; };
void i(struct K *k);
void j(struct K { int b; } k);
void k(struct N *a, struct N { int x; } b, struct N *c);
int l(struct O { int x; } a, struct O b) { struct O c = a; return b.x + c.x; }
struct R { int a; } n(void) { struct R r = { 1 }; return r; }
int s = sizeof(struct S { int a; });
int m(void) { return sizeof(struct O); }
