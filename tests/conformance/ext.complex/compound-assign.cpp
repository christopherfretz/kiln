// [expr.assign]/6 (`E1 op= E2` is `E1 = E1 op E2`, E1 evaluated once) on
// GNU `_Complex` objects in constant evaluation: GCC 14 folds `+=`, `-=`,
// `*=`, `/=` with a complex or real right operand, clang 19 and 22 fail
// with "subexpression not valid in a constant expression". kiln follows
// GCC (WP-380; spec/oven.md "Complex values"): computed in the operator's
// complex type and converted back to the left operand's type.
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang 19/22 do not fold compound assignment to a complex object
//@ diag: 37:15 error "static assertion expression is not an integral constant expression"
//@ diag: 36:37 note "division by zero"
//@ diag: 37:15 note "in call to 'zero()'"
using C = _Complex double;
using CF = _Complex float;
using CI = _Complex int;
constexpr C mk(double r, double i) { return __builtin_complex(r, i); }
constexpr C chain() {
  C c = 1.0, d = 2.0;
  c += d;
  c *= d;
  c -= 1.0;
  c /= d;
  return c;
}
static_assert(__real__ chain() == 2.5 && __imag__ chain() == 0);
constexpr C mul() { C c = mk(1, 2); c *= mk(3, -1); return c; }
static_assert(__real__ mul() == 5 && __imag__ mul() == 5);
// A `_Complex float` left operand: computed in `_Complex double`, then
// converted back.
constexpr CF mixed() { CF c = mk(1, 1); c += 1; c *= 2.0; c -= mk(0.5, 0.5); return c; }
static_assert(__real__ mixed() == 3.5f && __imag__ mixed() == 1.5f);
constexpr CI ints() { CI c = 3; c += 2; c *= c; c /= 5; return c; }
static_assert(__real__ ints() == 5 && __imag__ ints() == 0);
constexpr C ret() { C c = 1.0; (c += 1.0) += mk(0, 1); return c; }
static_assert(__real__ ret() == 2 && __imag__ ret() == 1);
constexpr int zero() { C c = 1.0; c /= 0.0; return 1; }
static_assert(zero());
