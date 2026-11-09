// Mangling differential: a temporary in a requires-clause or a dependent
// `decltype` mangles as the expression that creates it (clang
// `mangleExpression`: `MaterializeTemporaryExpr`, `CXXBindTemporaryExpr`
// and `ExprWithCleanups` are transparent; spec/abi.md §8.15, WP-980;
// [temp.over.link]/5). libc++ 22 chrono's `operator<<` for `sys_time`
// is constrained by `_Duration{1} < days{1}`, a class prvalue bound to
// `operator<`'s reference parameter.

struct D {
  long v;
  constexpr D(long x) : v(x) {}
};
constexpr bool operator<(const D& a, const D& b) { return a.v < b.v; }

// A class with a non-trivial destructor: the temporary is also bound
// (`CXXBindTemporaryExpr`) and the full-expression has cleanups.
struct B {
  long v;
  B(long x) : v(x) {}
  ~B() {}
};
bool operator<(const B& a, const B& b);
int take(const B&);

template <class T> struct W {};

template <class T>
  requires(T{1} < D{2})
void lt(W<T>) {}

template <class T>
  requires(!__is_same(T, int) && T{1} < D{2})
void lt2(W<T>) {}

template <class T>
  requires requires { T{1} < B{2}; }
void bound(W<T>) {}

template <class T>
auto ret(T t) -> decltype(take(B{2}) + t) { return t; }

template <class T>
  requires(sizeof(T) > 0 && D{1} < D{2})
void plain(W<T>) {}

int use() {
  lt(W<D>{});
  lt2(W<D>{});
  bound(W<B>{});
  plain(W<int>{});
  return ret(1);
}
