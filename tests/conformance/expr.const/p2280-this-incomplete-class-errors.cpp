// [expr.const]/10.1 (P2280R4) makes `this` in `this->f()` designate an
// object of unknown value, but in a member function's trailing return type
// inside its class's definition the class is incomplete ([class.mem]/8):
// there is no object yet, and the call is not constant, with clang 22's
// `this` note.
//@ expect: compile-fail
//@ strict-diags
//@ std: c++23
//@ diag: 14:19 error "non-type template argument is not a constant expression"
//@ diag: 14:19 note "implicit use of 'this' pointer is only allowed within the evaluation of a call to a 'constexpr' member function"
template <bool> struct Tag {};
struct Base { constexpr bool ok() const { return true; } };
struct Derived : Base {
  auto f() -> Tag<ok()>;
};
