// [expr.const]/10.1 (P2280R4): `get()` in a non-constexpr member function
// calls `get` on the object of unknown value `this` designates; its read
// of `m` is not constant. clang 22 rejects the call itself (it does not
// implement the `this` rule), so kiln's notes are its own: clang's
// `this` note at the read, and the call.
//@ expect: compile-fail
//@ strict-diags
//@ std: c++23
//@ xfail(clang): clang 22 rejects at the call's implicit `this`
//@ diag: 15:39 error "static assertion expression is not an integral constant expression"
//@ diag: 14:46 note "implicit use of 'this' pointer is only allowed within the evaluation of a call to a 'constexpr' member function"
//@ diag: 15:39 note "in call to 'this->get()'"
struct S {
  int m;  constexpr int get() const { return m; }
  void through_call() { static_assert(get() == 0); }
};
