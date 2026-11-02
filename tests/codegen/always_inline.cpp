// [temp.explicit]/12 and GNU `always_inline`: clang's -O0 pipeline runs
// the always-inliner (`AlwaysInlinerPass`, no lifetime markers), so every
// call to an `always_inline` definition is inlined and a discardable one
// left unused is deleted. An inline `always_inline` member under `extern
// template` is emitted `available_externally` (no comdat, not
// `dso_local`) for the inliner and never referenced; a non-inline one
// stays external and is kept. kiln emits `alwaysinline` (not `noinline
// optnone`) on such definitions and the driver runs `opt
// -passes=default<O0>` on the module.
// An `sret` callee's `noalias` result gets the inliner's alias scopes
// (first, so the metadata numbering does not depend on clang 22's
// `!nonnull` reference loads).
struct Big {
  long a, b, c, d;
};
[[gnu::always_inline]] inline Big make(long v) { return Big{v, v, v, v}; }
long use3() { return make(3).c; }

template <class T> struct A {
  __attribute__((always_inline)) int f(int x) { return x + 1; }
  [[gnu::always_inline]] inline int pick(int x) const;
  int g(int x) { return x + 2; }
};
template <class T> int A<T>::pick(int x) const {
  if (x > 0)
    return x;
  return -x;
}
extern template struct A<int>;
int use(A<int> &a) { return a.f(3) + a.g(4) + a.pick(-5); }

__attribute__((always_inline)) inline int twice(int x) { return x * 2; }
__attribute__((always_inline)) int kept(int x) { return twice(x) + 1; }
__attribute__((always_inline, noinline)) inline int never(int x) { return x; }
int use2() { return twice(5) + kept(6) + never(7); }
int (*addr())(int) { return &twice; }
struct E {
  ~E();
};
[[gnu::always_inline]] inline int with_dtor(int x) {
  E e;
  return x;
}
int use4(int x) {
  try {
    return with_dtor(x);
  } catch (...) {
    return 0;
  }
}
