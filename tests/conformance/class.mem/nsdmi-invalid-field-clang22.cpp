// [class.mem.general]/11, [class.base.init]/9: a field whose default
// member initializer fails is invalid: constructors, aggregate
// initializations and constant evaluations using it do not repeat the error.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:20 error "cannot initialize a member subobject of type 'int' with an lvalue of type 'const char[2]'"
//@ diag: 23:13 error "constexpr variable 'ca' must be initialized by a constant expression"
//@ diag: 23:13 note "subobject 'a' is not initialized"
//@ diag: 22:16 note "subobject declared here"
//@ diag: 24:20 error "cannot initialize a member subobject of type 'int' with an lvalue of type 'const char[2]'"
//@ diag: 25:6 error "default member initializer for 'a' needed within definition of enclosing class 'B' outside of member functions"
//@ diag: 24:16 note "default member initializer declared here"
//@ diag: 25:36 error "default member initializer for 'a' needed within definition of enclosing class 'B' outside of member functions"
//@ diag: 24:16 note "default member initializer declared here"
//@ diag: 26:20 error "cannot initialize a member subobject of type 'int' with an lvalue of type 'const char[2]'"
//@ diag: 27:21 error "default member initializer for 'a' needed within definition of enclosing class 'C' outside of member functions"
//@ diag: 26:16 note "default member initializer declared here"
//@ diag: 28:21 error "cannot initialize a member subobject of type 'int *' with an rvalue of type 'int'"
//@ diag: 30:38 error "cannot initialize a member subobject of type 'int' with an lvalue of type 'const char[2]'"
//@ requires: clang>=22
// clang 22 spelling of nsdmi-invalid-field.cpp (WP-394).
struct A { int a = "s"; int b; constexpr A() : b(1) {} };
constexpr A ca;
struct B { int a = "s"; };
B b1{}; B b2 = B(); constexpr B b3{};
struct C { int a = "s"; int c = 1; };
constexpr int k = C{}.c;
struct D { int *p = 1; D() = default; D(int) : D() {} };
D d(1);
template<class T> struct F { int a = "s"; T t{}; };
F<int> f; F<long> g;
