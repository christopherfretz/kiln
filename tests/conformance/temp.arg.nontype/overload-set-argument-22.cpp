// [temp.arg.nontype]/2, [over.over]/1: an overload set as a template
// argument for a non-type template parameter of pointer-to-function,
// reference-to-function or pointer-to-member-function type is resolved
// against the parameter type. When no function matches (or the parameter
// is not of such a type), "value of type '<overloaded function type>' is
// not implicitly convertible to 'int'" with a note at the parameter. A
// misspelled name before template arguments is corrected first ("did you
// mean '::val'?", the range of the name and a replacement fix-it).
// -19 is the clang 19 counterpart (no parameter note).
//@ requires: clang>=22
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 59:3 error "value of type '<overloaded function type>' is not implicitly convertible to 'int'"
//@ diag: 43:15 note "template parameter is declared here"
//@ diag: 60:3 error "value of type '<overloaded function type>' is not implicitly convertible to 'int'"
//@ diag: 43:15 note "template parameter is declared here"
//@ diag: 61:3 error "value of type '<overloaded function type>' is not implicitly convertible to 'int'"
//@ diag: 43:15 note "template parameter is declared here"
//@ diag: 62:3 error "value of type '<overloaded function type>' is not implicitly convertible to 'int'"
//@ diag: 43:15 note "template parameter is declared here"
//@ diag: 63:3 error "value of type '<overloaded function type>' is not implicitly convertible to 'int (*)(int)'"
//@ diag: 44:17 note "template parameter is declared here"
//@ diag: 64:3 error "value of type '<overloaded function type>' is not implicitly convertible to 'int *'"
//@ diag: 46:16 note "template parameter is declared here"
//@ diag: 67:3 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 39:5 note "'::val' declared here"
//@ diag: 67:3 error "value of type '<overloaded function type>' is not implicitly convertible to 'int'"
//@ diag: 43:15 note "template parameter is declared here"
//@ diag: 68:3 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 39:5 note "'::val' declared here"
//@ diag: 68:3 error "value of type '<overloaded function type>' is not implicitly convertible to 'int (*)(int)'"
//@ diag: 44:17 note "template parameter is declared here"
//@ diag: 69:3 error "use of undeclared identifier 'val2'; did you mean 'val'?"
//@ diag: 39:5 note "'val' declared here"
//@ diag: 69:3 error "value of type '<overloaded function type>' is not implicitly convertible to 'int (&)(int)'"
//@ diag: 45:17 note "template parameter is declared here"

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
