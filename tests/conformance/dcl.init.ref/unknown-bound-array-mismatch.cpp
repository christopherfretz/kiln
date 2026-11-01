// [dcl.init.ref]/4-5 (P0388R4): `T (&)[N]` does not bind an array of
// unknown bound, nor `T (&)[]` an array of a different element type or
// one whose binding would drop qualifiers (clang reports both as
// unrelated); no array temporary is created (/5.4.2)
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:7 error "non-const lvalue reference to type 'int[3]' cannot bind to a value of unrelated type 'int[]'"
//@ diag: 22:13 error "reference to type 'const int[3]' could not bind to an lvalue of type 'int[]'"
//@ diag: 23:8 error "reference to type 'int[3]' could not bind to an rvalue of type 'int[]'"
//@ diag: 25:7 error "non-const lvalue reference to type 'int[]' cannot bind to a value of unrelated type 'const int[3]'"
//@ diag: 26:16 error "volatile lvalue reference to type 'volatile int[]' cannot bind to a value of unrelated type 'const int[3]'"
//@ diag: 28:7 error "non-const lvalue reference to type 'int[][3]' cannot bind to a value of unrelated type 'int[3][2]'"
//@ diag: 29:8 error "non-const lvalue reference to type 'long[]' cannot bind to a value of unrelated type 'int[3]'"
//@ diag: 31:21 error "no matching function for call to 'f'"
//@ diag: 30:6 note "candidate function not viable: no known conversion from 'const int[3]' to 'int (&)[]' for 1st argument"




extern int u[];
int (&r3)[3] = u;
const int (&c3)[3] = u;
int (&&x3)[3] = static_cast<int(&&)[]>(u);
const int ca[3] = {};
int (&r4)[] = ca;
volatile int (&r5)[] = ca;
int m[3][2], a[3];
int (&r8)[][3] = m;
long (&r9)[] = a;
void f(int (&)[]);
void g() { (void)0; f(ca); }
