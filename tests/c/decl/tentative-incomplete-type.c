// C23 6.9.3p2, p5 (external object definitions): a tentative definition
// of an array of unknown size becomes a definition of a one-element array
// at the end of the translation unit (-Wtentative-definition-array); one
// of an incomplete struct type that is never completed is an error, and an
// internal-linkage one of incomplete non-array type is a constraint
// violation clang warns about (p3).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:17 warning "tentative definition of variable with internal linkage has incomplete non-array type 'struct S'"
//@ diag: 16:15 note "forward declaration of 'struct S'"
//@ diag: 15:5 warning "tentative array definition assumed to have one element"
//@ diag: 18:10 error "tentative definition has type 'struct T' that is never completed"
//@ diag: 18:8 note "forward declaration of 'struct T'"
//@ diag: 23:12 warning "tentative array definition assumed to have one element"
int a[];
static struct S s;
struct S { int x; };
struct T t;
struct U u;
struct U { int y; };
int b[];
int b[5];
static int c[];
int main(void) { return a[0] + s.x + u.y; }
