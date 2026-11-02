// [dcl.constexpr]/4 (clang applies it to every non-static member
// function): a constexpr constructor, destructor or member function of a
// class with virtual bases is an error at its definition, with a note per
// virtual base; the function is invalid, so `D` has no usable default
// constructor. A class template's pattern is checked once, not each
// specialization.
// Pinned to C++23: P3533R2 (C++26) allows virtual bases in constant evaluation.
//@ std: c++23
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:34 error "constexpr constructor not allowed in struct with virtual base class"
//@ diag: 25:12 note "virtual base class declared here"
//@ diag: 25:55 error "constexpr member function not allowed in struct with virtual base class"
//@ diag: 25:12 note "virtual base class declared here"
//@ diag: 25:89 error "constexpr member function not allowed in struct with virtual base class"
//@ diag: 25:12 note "virtual base class declared here"
//@ diag: 27:14 error "constructor for 'E' must explicitly initialize the base class 'D' which does not have a default constructor"
//@ diag: 25:8 note "'D' declared here"
//@ diag: 27:14 error "constexpr constructor not allowed in class with virtual base classes"
//@ diag: 26:11 note "virtual base class declared here"
//@ diag: 26:22 note "virtual base class declared here"
//@ diag: 28:52 error "constexpr constructor not allowed in struct with virtual base class"
//@ diag: 28:30 note "virtual base class declared here"
struct V { int x; };
struct D : virtual V { constexpr D() {} constexpr int f() const { return 1; } constexpr ~D() {} constexpr static int s() { return 0; } };
class E : virtual V, virtual D { constexpr E(int); };
constexpr E::E(int) {}
template<class T> struct F : virtual V { constexpr F() {} };
F<int> fi;
