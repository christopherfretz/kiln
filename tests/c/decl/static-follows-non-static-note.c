// C17 6.2.2p4, p7: a `static` redeclaration of a function with external
// linkage; the note says "previous definition" when the previous
// declaration is a definition (clang `getNoteDiagForInvalidRedeclaration`,
// WP-1312, WP-1306 follow-up).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:12 error "static declaration of 'f' follows non-static declaration"
//@ diag: 15:5 note "previous definition is here"
//@ diag: 18:12 error "static declaration of 'g' follows non-static declaration"
//@ diag: 17:5 note "previous declaration is here"
//@ diag: 20:12 error "static declaration of 'x' follows non-static declaration"
//@ diag: 19:5 note "previous definition is here"
//@ diag: 22:12 error "static declaration of 'y' follows non-static declaration"
//@ diag: 21:12 note "previous declaration is here"
int f(void) { return 0; }
static int f(void);
int g(void);
static int g(void) { return 1; }
int x = 1;
static int x;
extern int y;
static int y = 2;
