// [expr.cond]/7: "incompatible operand types" names each arm's type after
// the lvalue-to-rvalue, array-to-pointer and function-to-pointer
// conversions (a class arm keeps its cv-qualifiers).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:28 error "incompatible operand types ('int' and 'int (*)(int)')"
//@ diag: 19:28 error "incompatible operand types ('int' and 'int *')"
//@ diag: 20:28 error "incompatible operand types ('const A' and 'B')"
//@ diag: 21:28 error "incompatible operand types ('int' and 'int *')"
//@ diag: 22:28 error "incompatible operand types ('const char *' and 'double')"
int other(int);
int arr[3];
struct A {}; struct B {};
const A ca{}; B b; const volatile int cvi = 0; int *ip;

// The arms are named by their types after the array-to-pointer,
// function-to-pointer and lvalue-to-rvalue conversions.
void c1(bool c) { (void)(c ? 1 : other); }
void c2(bool c) { (void)(c ? 1 : arr); }
void c3(bool c) { (void)(c ? ca : b); }
void c4(bool c) { (void)(c ? cvi : ip); }
void c5(bool c) { (void)(c ? "abc" : 1.0); }
