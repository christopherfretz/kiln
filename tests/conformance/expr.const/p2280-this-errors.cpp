// [expr.const]/10.1 (P2280R4): `this` outside a call of a constexpr member
// function is constant only as the object expression of a class member
// access, and the object it designates has an unknown value: reading a
// member through it, implicitly or explicitly, an element of an array
// member, or `(*this).f()` (whose object expression is `*this`, not
// `this`; GCC 16.1 accepts it) are not constant, with clang 22's note at
// the `this`.
//@ expect: compile-fail
//@ strict-diags
//@ std: c++23
//@ diag: 23:31 error "static assertion expression is not an integral constant expression"
//@ diag: 23:31 note "implicit use of 'this' pointer is only allowed within the evaluation of a call to a 'constexpr' member function"
//@ diag: 24:40 error "static assertion expression is not an integral constant expression"
//@ diag: 24:40 note "use of 'this' pointer is only allowed within the evaluation of a call to a 'constexpr' member function"
//@ diag: 25:34 error "static assertion expression is not an integral constant expression"
//@ diag: 25:34 note "implicit use of 'this' pointer is only allowed within the evaluation of a call to a 'constexpr' member function"
//@ diag: 26:32 error "static assertion expression is not an integral constant expression"
//@ diag: 26:34 note "use of 'this' pointer is only allowed within the evaluation of a call to a 'constexpr' member function"
struct S {
  int m;
  int arr[3];
  constexpr int f() const { return 3; }
  void read() { static_assert(m == 0); }
  void read_explicit() { static_assert(this->m == 0); }
  void element() { static_assert(arr[1] == 0); }
  void deref() { static_assert((*this).f() == 3); }
};
