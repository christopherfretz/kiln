// C17 6.2.2p4, 6.2.7p2: every declaration of an identifier with external
// linkage, a block-scope `extern` one included, refers to one entity
// whose declarations must have compatible types, even when the earlier
// declaration is not visible (another block, or hidden by a local).
// clang `checkForConflictWithNonVisibleExternC` (every external
// declaration is "extern C" in C). Verified for WP-1311 (item 5, which
// is a C++ `extern "C"` gap; C already matched clang).
//@ std: c17
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:29 error "redeclaration of 'z' with a different type"
//@ diag: 19:27 note "previous declaration is here"
//@ diag: 22:30 error "conflicting types for 'fn'"
//@ diag: 21:27 note "previous declaration is here"
//@ diag: 24:37 error "redeclaration of 'w' with a different type"
//@ diag: 23:5 note "previous definition is here"
//@ diag: 26:6 error "redefinition of 'late' with a different type"
//@ diag: 25:27 note "previous declaration is here"
void a(void) { extern int z; }
void b(void) { extern float z; }
void c(void) { extern int fn(int); }
void d(void) { extern double fn(void); }
int w;
void e(void) { int w; { extern long w; } }
void f(void) { extern int late; }
long late;
