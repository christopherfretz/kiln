// C has no linkage specification (C++ [dcl.link] only): `extern "C"`
// is the storage class followed by a string literal where a declarator
// must start (C17 6.7.6), "expected identifier or '('" as clang. The
// declaration is skipped through its `;`, a braced body as a whole, and
// the declarations after it are parsed normally.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:8 error "expected identifier or '('"
//@ diag: 14:8 error "expected identifier or '('"
//@ diag: 15:8 error "expected identifier or '('"
//@ diag: 16:32 error "expected identifier or '('"
//@ diag: 19:27 error "expected identifier or '('"
extern "C" int f(void);
extern "C" { int g(void); }
extern "C++" int q;
__attribute__((unused)) extern "C" int h(void);
int k;
int *use(void) { return &k; }
void block(void) { extern "C" int lf(void); }
