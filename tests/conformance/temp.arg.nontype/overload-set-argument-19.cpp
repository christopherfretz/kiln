// [temp.arg.nontype]/2, [over.over]/1: overload-set-argument-22.cpp's forms
// under clang 19: no "template parameter is declared here" note, and a
// corrected template-id that names one function is shown with its type.
//@ requires: clang<22
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 42:3 error "value of type '<overloaded function type>' is not implicitly convertible to 'int'"
//@ diag: 43:3 error "value of type '<overloaded function type>' is not implicitly convertible to 'int'"
//@ diag: 44:3 error "value of type '<overloaded function type>' is not implicitly convertible to 'int'"
//@ diag: 45:3 error "value of type '<overloaded function type>' is not implicitly convertible to 'int'"
//@ diag: 46:3 error "value of type '<overloaded function type>' is not implicitly convertible to 'int (*)(int)'"
//@ diag: 47:3 error "value of type '<overloaded function type>' is not implicitly convertible to 'int *'"
//@ diag: 50:3 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 22:5 note "'::val' declared here"
//@ diag: 50:3 error "value of type 'int (int)' is not implicitly convertible to 'int'"
//@ diag: 51:3 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 22:5 note "'::val' declared here"
//@ diag: 52:3 error "use of undeclared identifier 'val2'; did you mean 'val'?"
//@ diag: 22:5 note "'val' declared here"

int val(int);
int f(int);
int f(double);
template <class T> int tf(T);
template <int N> struct A {};
template <int (*F)(int)> struct C {};
template <int (&F)(int)> struct R {};
template <int *Q> struct P {};
struct S { int m(int); int m(double); };
template <int (S::*Q)(int)> struct M {};

// Overload sets resolve against the parameter type.
C<f> c1;
C<&f> c2;
C<(f)> c3;
C<tf<int>> c4;
R<f> r1;
M<&S::m> m1;

// No match: the overloaded function type is not convertible.
A<f> a1;
A<tf<int>> a2;
A<(tf<int>)> a3;
A<&tf<int>> a4;
C<tf<int, int>> c5;
P<f> p1;

// A misspelled template-name in a template argument.
A<val<int>> a5;
C<val<int>> c6;
R<val2<int>> r2;
