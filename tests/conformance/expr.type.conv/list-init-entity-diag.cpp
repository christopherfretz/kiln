// [expr.type.conv]/2: `T{x}` list-initializes a temporary, and
// [expr.new]/24 initializes the new object: a failed conversion names
// the entity "a value" / "a new value", not "a variable" (clang
// `InitializedEntity::EK_Temporary` / `EK_New`).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:21 error "cannot initialize a value of type 'P' (aka 'int *') with an lvalue of type 'double'"
//@ diag: 20:21 warning "too many braces around scalar initializer"
//@ diag: 20:22 error "cannot initialize a value of type 'P' (aka 'int *') with an lvalue of type 'double'"
//@ diag: 21:21 error "cannot initialize a value of type 'E' with an rvalue of type 'double *'"
//@ diag: 22:46 error "cannot initialize a value of type 'int *' with an lvalue of type 'double'"
//@ diag: 23:15 note "in instantiation of function template specialization 'g<int *>' requested here"
//@ diag: 24:17 error "cannot initialize a new value of type 'P' (aka 'int *') with an lvalue of type 'double'"
//@ diag: 25:19 error "cannot initialize a new value of type 'P' (aka 'int *') with an lvalue of type 'double'"
//@ diag: 26:20 error "cannot initialize a variable of type 'int *' with an lvalue of type 'double'"
using P = int*;
double d;
enum class E : int {};
void f1() { (void)P{d}; }
void f2() { (void)P{{d}}; }
void f3() { (void)E{&d}; }
template<class T> void g(double x) { (void)T{x}; }
template void g<int*>(double);
void f4() { new P(d); }
void f5() { new P{d}; }
void f6() { int* x{d}; }
