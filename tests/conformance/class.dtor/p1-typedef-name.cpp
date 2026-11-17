// [class.dtor]/1: in a destructor declaration the name after ~ is the
// injected-class-name or the class name; a typedef or alias naming the class
// is an error by default in clang (-Wdtor-typedef), also out of class.
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:26 error "destructor cannot be declared using a typedef 'A2' (aka 'A') of the class name [-Wdtor-typedef]"
//@ diag: 12:26 error "destructor cannot be declared using a type alias 'B2' (aka 'B') of the class name [-Wdtor-typedef]"
//@ diag: 13:39 error "destructor cannot be declared using a typedef 'C2' (aka 'C') of the class name [-Wdtor-typedef]"
//@ diag: 14:44 error "destructor cannot be declared using a typedef 'E2' (aka 'E<T>') of the class name [-Wdtor-typedef]"
struct A { typedef A A2; ~A2(); };
struct B { using B2 = B; ~B2(); };
struct C { ~C(); }; typedef C C2; C2::~C2() {}
template<class T> struct E { typedef E E2; ~E2(); };
