// GNU `_Complex` in constant expressions ([expr.const]): `++`/`--` step the
// real part, `==`/`!=` with a real operand, casts between element types,
// `__real`/`__imag` lvalues, `__builtin_complex`, `?:`, comma, complex
// objects in arrays, members and through references, and the operators
// sema rejects on a complex operand ([expr.rel]/2, [expr.spaceship]/4)
//@ expect: compile-fail
//@ strict-diags
//@ args: -Wno-unused-value
//@ diag: 42:24 error "invalid operands to binary expression ('const _Complex double' and 'const _Complex double')"
//@ diag: 43:24 error "invalid operands to binary expression ('const _Complex double' and 'double')"
//@ diag: 44:24 error "invalid operands to binary expression ('const _Complex int' and 'int')"
constexpr _Complex double c1 = {1.0, 2.0};
constexpr _Complex int ci = {1, 2};
constexpr _Complex double f1() {
  _Complex double c = {1, 2};
  ++c; c++; --c; c--; ++c;
  return c;
}
static_assert(__real__ f1() == 2.0 && __imag__ f1() == 2.0);
static_assert(c1 == __builtin_complex(1.0, 2.0) && 1.0 != c1 && !(c1 == 1.0));
constexpr _Complex float cf = c1;
constexpr _Complex int cii = __builtin_complex(3.75, -2.5);
static_assert(__real__ cii == 3 && __imag__ cii == -2);
static_assert(cf == ci && (_Complex double)ci == c1);
constexpr _Complex double f2(bool b) {
  _Complex double a[2] = {c1, {}};
  _Complex double &r = b ? a[0] : a[1];
  __imag__ r = 7;
  __real__ a[1] = (a[0], 5.0);
  return a[0] + a[1];
}
static_assert(__real__ f2(true) == 6.0 && __imag__ f2(true) == 7.0);
static_assert(__real__ f2(false) == 6.0 && __imag__ f2(false) == 9.0);
struct S { _Complex double m; };
constexpr S s{__builtin_complex(3.0, 4.0)};
static_assert(__imag__ (s.m * s.m) == 24.0 && (true ? s.m : c1) != c1);
static_assert((c1, ci) == ci && (1, c1) == c1);
static_assert(!c1 == false && (c1 && ci) && (c1 * 0.0 || ci));
constexpr _Complex int ci2 = {1, -2}, ci3 = {-1, -2};
static_assert(-ci == ci3 && ~ci == ci2);
static_assert(+c1 == c1);
constexpr bool r1 = c1 < c1;
constexpr bool r2 = c1 >= 1.0;
constexpr auto r3 = ci % 2;
