// [expr.add]/4, [dcl.ref]/5: clang 22 checks a reference data member as it
// binds, as it does a reference variable (WP-948): one bound to the object
// past the end of an array is not constant, in aggregate and designated
// initialization, a new-expression, a temporary, a default member
// initializer (noted at the constructor), a mem-initializer and a base's
// aggregate initialization. clang 19 checks only the final value
// (ref-member-past-end-19.cpp). WP-1021.
//@ std: c++26
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 45:15 error "constexpr variable 'k1' must be initialized by a constant expression"
//@ diag: 44:38 note "read of dereferenced one-past-the-end pointer is not allowed in a constant expression"
//@ diag: 45:20 note "in call to 'f1(&a[0])'"
//@ diag: 47:15 error "constexpr variable 'k2' must be initialized by a constant expression"
//@ diag: 46:43 note "read of dereferenced one-past-the-end pointer is not allowed in a constant expression"
//@ diag: 47:20 note "in call to 'f2(&a[0])'"
//@ diag: 49:15 error "constexpr variable 'k3' must be initialized by a constant expression"
//@ diag: 48:47 note "read of dereferenced one-past-the-end pointer is not allowed in a constant expression"
//@ diag: 49:20 note "in call to 'f3(&a[0])'"
//@ diag: 50:41 warning "left operand of comma operator has no effect [-Wunused-value]"
//@ diag: 51:15 error "constexpr variable 'k4' must be initialized by a constant expression"
//@ diag: 50:43 note "read of dereferenced one-past-the-end pointer is not allowed in a constant expression"
//@ diag: 51:20 note "in call to 'f4(&a[0])'"
//@ diag: 53:15 error "constexpr variable 'k5' must be initialized by a constant expression"
//@ diag: 41:8 note "read of dereferenced one-past-the-end pointer is not allowed in a constant expression"
//@ diag: 52:24 note "in call to 'D()'"
//@ diag: 53:20 note "in call to 'f5()'"
//@ diag: 55:15 error "constexpr variable 'k6' must be initialized by a constant expression"
//@ diag: 42:56 note "read of dereferenced one-past-the-end pointer is not allowed in a constant expression"
//@ diag: 54:24 note "in call to 'C(&a[0])'"
//@ diag: 55:20 note "in call to 'f6()'"
//@ diag: 57:15 error "constexpr variable 'k7' must be initialized by a constant expression"
//@ diag: 56:39 note "read of dereferenced one-past-the-end pointer is not allowed in a constant expression"
//@ diag: 57:20 note "in call to 'f7(&a[0])'"
//@ diag: 58:13 error "constexpr variable 'g' must be initialized by a constant expression"
//@ diag: 58:15 note "read of dereferenced one-past-the-end pointer is not allowed in a constant expression"
constexpr int a[2] = {1, 2};
constexpr const int *end = a + 2;
struct R { const int &r; };
struct D { const int &r = *end; };
struct C { const int &r; constexpr C(const int *p) : r(p[2]) {} };
struct B : R {};
constexpr int f1(const int *p) { R x{p[2]}; return 0; }
constexpr int k1 = f1(a);
constexpr int f2(const int *p) { R x{.r = p[2]}; return 0; }
constexpr int k2 = f2(a);
constexpr int f3(const int *p) { R *q = new R{p[2]}; delete q; return 0; }
constexpr int k3 = f3(a);
constexpr int f4(const int *p) { return R{p[2]}, 0; }
constexpr int k4 = f4(a);
constexpr int f5() { D d; return 0; }
constexpr int k5 = f5();
constexpr int f6() { C c(a); return 0; }
constexpr int k6 = f6();
constexpr int f7(const int *p) { B b{{p[2]}}; return 0; }
constexpr int k7 = f7(a);
constexpr R g{*end};
