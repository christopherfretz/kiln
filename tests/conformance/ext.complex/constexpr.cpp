// Constant evaluation of GNU _Complex values (clang 19 ExprConstant
// ComplexExprEvaluator): parts, ++/-- on the real part, Annex G
// multiplication/division, comparisons, conversions, __builtin_complex
// (WP-211).
//@ expect: compile-ok
using C = _Complex double;
using CI = _Complex int;
using CF = _Complex float;
constexpr C mk(double r, double i) { C c = 0; __real__ c = r; __imag__ c = i; return c; }
constexpr bool steps() {
  C a = mk(1, 5);
  C o = a++;
  if (__real__ o != 1 || __imag__ o != 5) return false;
  if (__real__ a != 2 || __imag__ a != 5) return false;
  C p = --a;
  return __real__ p == 1 && __imag__ p == 5;
}
static_assert(steps());
constexpr int isteps() { CI a = 0; __imag__ a = 7; (++a)++; return __real__ a * 10 + __imag__ a; }
static_assert(isteps() == 27);
constexpr bool fsteps() { CF f = 1.5f; f--; return __real__ f == 0.5f && __imag__ f == 0; }
static_assert(fsteps());
static_assert(__real__ (mk(1, 2) * mk(3, 4)) == -5 && __imag__ (mk(1, 2) * mk(3, 4)) == 10);
static_assert(__real__ (mk(-5, 10) / mk(3, 4)) == 1 && __imag__ (mk(-5, 10) / mk(3, 4)) == 2);
static_assert(mk(1, 2) == mk(1, 2) && mk(1, 2) != mk(1, 3) && mk(4, 0) == 4);
static_assert(__real__ (mk(1, 2) + 1) == 2 && __imag__ (2.0 - mk(1, 2)) == -2);
static_assert(__imag__ ~mk(1, 2) == -2 && __real__ -mk(1, 2) == -1);
constexpr CI ia = CI(7) / CI(2);
static_assert(__real__ ia == 3 && __imag__ ia == 0);
constexpr CI fromf = (CI)mk(2.9, -1.5);
static_assert(__real__ fromf == 2 && __imag__ fromf == -1);
constexpr C bc = __builtin_complex(1.0, 2.0);
static_assert(__imag__ bc == 2.0 && bool(bc) && !bool(C(0)));
constexpr CF lst = {1.0f, 2.0f};
static_assert(__imag__ lst == 2.0f);
constexpr double re = (double)mk(6, 7);
static_assert(re == 6);
