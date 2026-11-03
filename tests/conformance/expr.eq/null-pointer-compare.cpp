// [conv.ptr]/1, [basic.compound]/3, [dcl.ref]/5, [expr.prim.this]: a pointer
// to an object or function is never a null pointer value, nor is the
// address of a reference or 'this', so comparing one with a null pointer
// constant always gives the same result (clang -Wtautological-pointer-compare
// and -Wtautological-undefined-compare, DiagnoseAlwaysNonNullPointer). The
// comparisons sit in namespace-scope initializers, member functions and
// templates; an instantiation warns again. Weak declarations, a parenthesized
// or macro-body operand, '&f' and non-null-constant operands are not diagnosed.
//@ expect: compile-ok
//@ std: c++20
//@ strict-diags
//@ diag: 50:76 warning "'this' pointer cannot be null in well-defined C++ code; comparison may be assumed to always evaluate to false [-Wtautological-undefined-compare]"
//@ diag: 50:114 warning "comparison of address of 'this->m' not equal to a null pointer is always true [-Wtautological-pointer-compare]"
//@ diag: 55:13 warning "comparison of address of 'x' equal to a null pointer is always false [-Wtautological-pointer-compare]"
//@ diag: 58:23 warning "comparison of address of 'x' equal to a null pointer is always false [-Wtautological-pointer-compare]"
//@ diag: 59:16 warning "comparison of array 'arr' equal to a null pointer is always false [-Wtautological-pointer-compare]"
//@ diag: 60:12 warning "comparison of address of 'R::x' not equal to a null pointer is always true [-Wtautological-pointer-compare]"
//@ diag: 61:14 warning "comparison of address of 's.m' equal to a null pointer is always false [-Wtautological-pointer-compare]"
//@ diag: 62:13 warning "comparison of array 's.a' equal to a null pointer is always false [-Wtautological-pointer-compare]"
//@ diag: 63:12 warning "reference cannot be bound to dereferenced null pointer in well-defined C++ code; comparison may be assumed to always evaluate to false [-Wtautological-undefined-compare]"
//@ diag: 46:6 note "'rf' returns a reference"
//@ diag: 64:12 warning "comparison of function 'fi' equal to a null pointer is always false [-Wtautological-pointer-compare]"
//@ diag: 64:12 note "prefix with the address-of operator to silence this warning"
//@ diag: 64:12 note "suffix with parentheses to turn this into a function call"
//@ diag: 65:12 warning "comparison of function 'fp' equal to a null pointer is always false [-Wtautological-pointer-compare]"
//@ diag: 65:12 note "prefix with the address-of operator to silence this warning"
//@ diag: 65:12 note "suffix with parentheses to turn this into a function call"
//@ diag: 66:12 warning "comparison of function 'fp' equal to a null pointer is always false [-Wtautological-pointer-compare]"
//@ diag: 66:12 note "prefix with the address-of operator to silence this warning"
//@ diag: 66:12 note "suffix with parentheses to turn this into a function call"
//@ diag: 67:12 warning "comparison of function 'fi' equal to a null pointer is always false [-Wtautological-pointer-compare]"
//@ diag: 67:12 note "prefix with the address-of operator to silence this warning"
//@ diag: 73:12 warning "comparison of function 'S::sf' equal to a null pointer is always false [-Wtautological-pointer-compare]"
//@ diag: 73:12 note "prefix with the address-of operator to silence this warning"
//@ diag: 75:13 warning "comparison of address of 'x' equal to a null pointer is always false [-Wtautological-pointer-compare]"
//@ diag: 76:13 warning "comparison of address of 'x' equal to a null pointer is always false [-Wtautological-pointer-compare]"
//@ diag: 81:39 warning "comparison of address of 'x' not equal to a null pointer is always true [-Wtautological-pointer-compare]"
//@ diag: 84:52 warning "reference cannot be bound to dereferenced null pointer in well-defined C++ code; comparison may be assumed to always evaluate to false [-Wtautological-undefined-compare]"
//@ diag: 80:43 warning "reference cannot be bound to dereferenced null pointer in well-defined C++ code; comparison may be assumed to always evaluate to true [-Wtautological-undefined-compare]"
//@ diag: 83:11 note "in instantiation of function template specialization 't1<int>' requested here"
//@ diag: 81:39 warning "comparison of address of 'x' not equal to a null pointer is always true [-Wtautological-pointer-compare]"
//@ diag: 83:20 note "in instantiation of function template specialization 't2<int>' requested here"
namespace R { int x; }
int x, arr[2];
int &r = x;
int &rf();
void fn();
int fi();
int *fp();
struct S { int m; int a[2]; void mf(); static void sf(); bool t() { return this == nullptr; } bool u() { return &m != 0; } };
S s;
extern int wk __attribute__((weak));
#define ISNULL(p) ((p) == nullptr)
#define ADDRX &x
bool c1 = (&x) == nullptr;
bool c2 = &(x) == nullptr;
bool c3 = (arr) == nullptr;
bool c4 = nullptr == &x;
bool c5 = 0 == arr;
bool c6 = &R::x != 0;
bool c7 = &s.m == nullptr;
bool c8 = s.a == nullptr;
bool c9 = &rf() == nullptr;
bool c10 = fi == 0;
bool c11 = fp == 0;
bool c12 = fp == nullptr;
bool c13 = fi == nullptr;
bool c14 = &fn == nullptr;
bool c15 = &wk == nullptr;
bool c16 = ISNULL(&x);
bool c17 = ADDRX == nullptr;
bool c18 = &S::sf == nullptr;
bool c19 = S::sf == nullptr;
bool c20 = &x == (int*)0;
bool c21 = &x == __null;
bool c22 = &x == 0L;
bool c23 = arr != (void*)0;
bool c25 = &arr[1] == nullptr;
bool c26 = "abc" == nullptr;
template<class T> bool t1(T &a) { return &a != nullptr; }
template<class T> bool t2() { return &x != nullptr; }
template<class T> bool t3(T a) { return a != nullptr; }
bool d1 = t1(x) && t2<int>() && t3(&x);
void h(int &pr, int pa[2], int (&ra)[2]) { (void)(&pr == nullptr); (void)(pa == nullptr); (void)(ra == nullptr); }
