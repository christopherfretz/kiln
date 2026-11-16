// [expr.new]/20 with [expr.const]/10 (C++26, P2747R2): `new (0) X` and
// `new (nullptr) X` select the reserved placement form, and constructing
// at a null address is not a constant expression. Found running GCC's C++
// testsuite.
//@ expect: compile-fail
//@ requires: clang>=22
//@ strict-diags
//@ std: c++26
//@ diag: 20:15 error "static assertion expression is not an integral constant expression"
//@ diag: 17:34 note "construction of dereferenced null pointer is not allowed in a constant expression"
//@ diag: 20:15 note "in call to 'by_zero()'"
//@ diag: 21:15 error "static assertion expression is not an integral constant expression"
//@ diag: 18:37 note "construction of dereferenced null pointer is not allowed in a constant expression"
//@ diag: 21:15 note "in call to 'by_nullptr()'"
#include <new>
struct X { int a; };
constexpr int by_zero() { X *p = new (0) X{1}; return p->a; }
constexpr int by_nullptr() { X *p = new (nullptr) X{1}; return p->a; }
constexpr int at_object() { X x{0}; X *p = new (&x) X{2}; return p->a; }
static_assert(by_zero() == 1);
static_assert(by_nullptr() == 1);
static_assert(at_object() == 2);
