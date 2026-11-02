// [temp.variadic]/5, [expr.sub]/1, [over.sub]/1: a subscript's
// expression-list may be a pack expansion or several expressions; in a
// template it is a dependent `operator[]` call (clang
// `CreateOverloadedArraySubscriptExpr`), resolved per instantiation as a
// multidimensional, one-argument or zero-argument `operator[]` or a
// built-in subscript (libc++ mdspan `mds[args...]`). Verified against
// clang 19.
//@ expect: run
struct M {
  int v[2][3];
  constexpr int& operator[](int i, int j) { return v[i][j]; }
  constexpr int& operator[](int i) { return v[0][i]; }
  constexpr int operator[]() const { return 42; }
};
template <class T, class... I> constexpr int get(T m, I... is) { return m[is...]; }
template <class T> constexpr int two(T m) { return m[1, 0]; }
template <class... I> constexpr int builtin(I... is) {
  int a[4] = {1, 2, 3, 4};
  return a[is...];
}
template <class... I> constexpr int fold(M m, I... is) { return (m[is, 1] + ... + 0); }
template <class... I> constexpr int set(M& m, I... is) { return m[is...] = 9; }

constexpr M m{{{1, 2, 3}, {4, 5, 6}}};
static_assert(get(m, 1, 2) == 6);
static_assert(get(m, 2) == 3);
static_assert(get(M{}) == 42);
static_assert(two(m) == 4);
static_assert(builtin(3) == 4);
static_assert(fold(m, 0, 1) == 7);

int main() {
  M r{{{1, 2, 3}, {4, 5, 6}}};
  int i = 1, j = 2;
  if (get(r, i, j) != 6 || get(r, j) != 3 || builtin(j) != 3) return 1;
  if (set(r, 0, 0) != 9 || r.v[0][0] != 9) return 2;
  return fold(r, 0, 1) == 7 ? 0 : 3;
}
