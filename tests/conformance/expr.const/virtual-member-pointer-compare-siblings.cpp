// [expr.eq]/4.3: a comparison involving a pointer to a virtual member
// function has an unspecified result, with '!=' and against a non-virtual
// member (WP-948 siblings of virtual-member-pointer-compare).
//@ std: c++23
//@ expect: compile-fail
//@ diag: 11:16 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 11:26 note "comparison of pointer to virtual member function 'f' has unspecified value"
//@ diag: 12:16 error "constexpr variable 'b' must be initialized by a constant expression"
//@ diag: 12:26 note "comparison of pointer to virtual member function 'f' has unspecified value"
struct A { virtual int f(); virtual int g(); int h(); };
constexpr bool a = &A::f != &A::g;
constexpr bool b = &A::f == &A::h;
