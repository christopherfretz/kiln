// [meta.const.eval]/4 (P2641R4), [expr.const]/25: a consteval constructor
// that is an immediate invocation initializing a variable may be handed a
// pointer to that variable; the variable is still being initialized, so
// `__builtin_is_within_lifetime` on it fails with clang's kind 3 ("an object
// whose lifetime is being evaluated").  This holds for namespace-scope and
// local constexpr variables, constinit variables and every initializer
// form; a variable that is not constexpr reads as a non-constexpr variable.
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 55:13 error "constexpr variable 'c1' must be initialized by a constant expression"
//@ diag: 53:34 note "'__builtin_is_within_lifetime' cannot be called with a pointer to an object whose lifetime has not yet begun"
//@ diag: 55:13 note "in call to 'C(&c1)'"
//@ diag: 55:13 error "call to consteval function 'C::C' is not a constant expression"
//@ diag: 53:34 note "initializer of 'c1' is not a constant expression"
//@ diag: 55:13 note "in call to 'C(&c1)'"
//@ diag: 55:13 note "declared here"
//@ diag: 56:13 error "constexpr variable 'c2' must be initialized by a constant expression"
//@ diag: 53:34 note "'__builtin_is_within_lifetime' cannot be called with a pointer to an object whose lifetime has not yet begun"
//@ diag: 56:13 note "in call to 'C(&c2)'"
//@ diag: 56:13 error "call to consteval function 'C::C' is not a constant expression"
//@ diag: 53:34 note "initializer of 'c2' is not a constant expression"
//@ diag: 56:13 note "in call to 'C(&c2)'"
//@ diag: 56:13 note "declared here"
//@ diag: 57:13 error "constexpr variable 'c3' must be initialized by a constant expression"
//@ diag: 53:34 note "'__builtin_is_within_lifetime' cannot be called with a pointer to an object whose lifetime has not yet begun"
//@ diag: 57:18 note "in call to 'C(&c3)'"
//@ diag: 57:18 error "call to consteval function 'C::C' is not a constant expression"
//@ diag: 53:34 note "initializer of 'c3' is not a constant expression"
//@ diag: 57:18 note "in call to 'C(&c3)'"
//@ diag: 57:13 note "declared here"
//@ diag: 58:13 error "variable does not have a constant initializer"
//@ diag: 58:1 note "required by 'constinit' specifier here"
//@ diag: 53:34 note "'__builtin_is_within_lifetime' cannot be called with a pointer to an object whose lifetime has not yet begun"
//@ diag: 58:13 note "in call to 'C(&c4)'"
//@ diag: 58:13 error "call to consteval function 'C::C' is not a constant expression"
//@ diag: 53:34 note "read of non-constexpr variable 'c4' is not allowed in a constant expression"
//@ diag: 58:13 note "in call to 'C(&c4)'"
//@ diag: 58:13 note "declared here"
//@ diag: 59:3 error "call to consteval function 'C::C' is not a constant expression"
//@ diag: 53:34 note "read of non-constexpr variable 'c5' is not allowed in a constant expression"
//@ diag: 59:3 note "in call to 'C(&c5)'"
//@ diag: 59:3 note "declared here"
//@ diag: 60:24 error "constexpr variable 'c6' must be initialized by a constant expression"
//@ diag: 53:34 note "'__builtin_is_within_lifetime' cannot be called with a pointer to an object whose lifetime has not yet begun"
//@ diag: 60:24 note "in call to 'C(&c6)'"
//@ diag: 60:24 error "call to consteval function 'C::C' is not a constant expression"
//@ diag: 53:34 note "initializer of 'c6' is not a constant expression"
//@ diag: 60:24 note "in call to 'C(&c6)'"
//@ diag: 60:24 note "declared here"
struct C {
  int m;
  consteval C(const C* self) : m(__builtin_is_within_lifetime(self)) {}
};
constexpr C c1(&c1);
constexpr C c2{&c2};
constexpr C c3 = C(&c3);
constinit C c4(&c4);
C c5(&c5);
void f() { constexpr C c6(&c6); }
