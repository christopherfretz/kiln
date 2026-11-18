// [expr.cond]/5 in a constant evaluation: a glvalue conditional of
// bit-fields reads and writes the selected operand's bit-field (WP-1692:
// the Oven gave up on the form with an internal error before). Reads,
// assignments whose value is re-read from the bit-field, `bool`,
// enumeration and `unsigned` bit-fields, a bit-field and a local `int`,
// a right operand that changes the condition, a nested conditional with
// postfix `++`, and a `const int&` bound to a copy. The values are GCC
// 16.1's (it runs the same function at run time and in constant
// evaluation alike); clang 22 agrees.
//@ expect: compile-ok
enum E { E0, E1, E2, E3 };
struct S { int p : 4; int q : 12; unsigned r : 3; bool b : 1; E e : 2; int : 0; unsigned z : 5; int w; };
struct R { int v[8]; };
constexpr R f(bool c) {
  S s{-2, 1000, 5, true, E2, 17, 7};
  S t{1, 2, 3, false, E1, 4, 9};
  int loc = 50;
  R out{};
  out.v[0] = c ? s.p : s.q;
  out.v[1] = ((c ? s.p : t.q) = 9);
  out.v[2] = ((c ? s.r : t.z) = 77);
  out.v[3] = (c ? s.b : t.b) = false;
  out.v[4] = ((c ? s.e : t.e) = E3);
  bool cc = c;
  (cc ? s.q : loc) += (cc = !cc, 5);
  int post = (c ? (c ? s.p : loc) : (c ? t.q : s.w))++;
  const int &cr = c ? s.q : t.p;
  s.q = 0; t.p = 0;
  out.v[5] = post * 1000 + cr;
  out.v[6] = s.p * 100000 + s.q * 1000 + (int)s.r * 100 + t.q * 10 + loc;
  out.v[7] = s.w + t.w + (int)t.z + (int)s.b + (int)t.e + (int)s.e;
  return out;
}
constexpr R a = f(true), b = f(false);
static_assert(a.v[0] == -2 && b.v[0] == 1000, "");
static_assert(a.v[1] == -7 && b.v[1] == 9, "");
static_assert(a.v[2] == 5 && b.v[2] == 13, "");
static_assert(a.v[3] == 0 && b.v[3] == 0, "");
static_assert(a.v[4] == 3 && b.v[4] == 3, "");
static_assert(a.v[5] == -6000 && b.v[5] == 7001, "");
static_assert(a.v[6] == -599425 && b.v[6] == -199360, "");
static_assert(a.v[7] == 24 && b.v[7] == 36, "");
