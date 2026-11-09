// [dcl.constexpr]/6: constructs the Oven cannot evaluate yet (a case label
// nested in a loop of the switch body, a bit cast of a class with a base)
// leave a function possibly constant: an Oven limitation ("kiln: ... not
// supported yet") is never proof of never constant (WP-962). clang 19
// accepts both with -Winvalid-constexpr.
//@ std: c++17
//@ args: -Winvalid-constexpr
//@ strict-diags
//@ expect: compile-ok
constexpr int duff(int n) {
  int r = 0;
  switch (n % 2) {
  case 0:
    do {
      ++r;
    case 1:
      ++r;
    } while (--n > 0);
  }
  return r;
}
struct B { int a; };
struct D : B { int b; };
constexpr long bc(D d) { return __builtin_bit_cast(long, d); }
// No unknowns: the whole body is evaluated, and fails in the Oven.
constexpr int duff3() { return duff(3); }
constexpr long bc0() { return __builtin_bit_cast(long, D{{1}, 2}); }
