// [class.temporary]/6, [stmt.ranged]/1, [expr.const], [temp.inst]:
// the temporaries an instantiated constexpr function template's
// declarations extend are its own (WP-3271), and constant evaluation sees
// each live until its declaration's lifetime ends: a reference bound to a
// member of a temporary, the reference members of an aggregate, a
// structured binding and a range-based `for` over a member of a
// temporary. Two specializations, evaluated in either order.
//@ std: c++20
//@ strict-diags

struct S {
  int a[2];
  constexpr S(int x) : a{x, x + 1} {}
  constexpr const int *begin() const { return a; }
  constexpr const int *end() const { return a + 2; }
};
struct A {
  const S &s;
  const int &i;
};
struct W {
  S s;
};
struct P {
  int x, y;
};

template <class T> constexpr int f(T k) {
  int n = 0;
  for (int x : W{S(k)}.s) n += x;
  const int &i = S(k + 2).a[1];
  A agg{S(k + 3), int(k) + 4};
  const auto &[p, q] = P{int(k) + 5, int(k) + 6};
  return n + i + agg.s.a[0] + agg.i + p + q;
}

static_assert(f<long>(1) == 29);
static_assert(f<int>(0) == 22);
static_assert(f<long>(2) == 36);
