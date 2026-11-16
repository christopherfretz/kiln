// [class.copy.elision]/1.3 (C++14), [expr.context]/1, [except.spec]/6: in
// an unevaluated operand too, the elidable copy of a prvalue that
// initializes an object or a parameter of its class is a use of the copy
// or move constructor: an inaccessible or deleted one is an error, and a
// potentially-throwing one makes `noexcept` false. The copy is not
// odr-used there, so a constructor template's definition is not
// instantiated. Found running GCC's C++ testsuite.
//@ std: c++14
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:19 error "calling a private constructor of class 'A'"
//@ diag: 17:28 note "declared private here"
//@ diag: 20:12 error "calling a private constructor of class 'A'"
//@ diag: 17:28 note "declared private here"
//@ diag: 22:16 error "functional-style cast from 'B' to 'B' uses deleted function"
//@ diag: 21:19 note "candidate constructor has been explicitly deleted"
struct A { A() {} private: A(const A &); };
void g(A);
bool b = noexcept(A(A()));
decltype(g(A())) *p;
struct B { B() {} B(const B &) = delete; };
int t = sizeof(B(B{}));
template<class T> struct C { C() {} C(const C &) { T::fail(); } };
int u = sizeof(C<int>(C<int>{}));
