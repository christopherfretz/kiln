// [over.match.best.general]/2.10: of two otherwise equally good
// candidates, the one generated from a deduction-guide beats one
// generated from a constructor.
//@ strict-diags
template<class T> struct W {
  T *p;
  constexpr W(T *x) : p(x) {}
};
template<class T> W(T *) -> W<const T>;
int i;
W w(&i);
static_assert(__is_same(decltype(w), W<const int>));
// A better match still wins regardless of origin.
template<class T> struct V { V(T); };
template<class T> V(T, int) -> V<T *>;
V v(1);
static_assert(__is_same(decltype(v), V<int>));
// A non-template guide beats a template one ([over.match.best.general]/2.4).
template<class T> struct U { U(T); };
U(int) -> U<long>;
U u(1);
static_assert(__is_same(decltype(u), U<long>));
U u2(1.0);
static_assert(__is_same(decltype(u2), U<double>));
