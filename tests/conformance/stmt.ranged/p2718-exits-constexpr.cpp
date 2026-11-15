// The constant-evaluation twin of p2718-exits-run.cpp: from C++23 the
// for-range-initializer's temporaries persist until the loop is left by
// its end, `break` or `return` ([class.temporary]/7, P2718R0), and are
// destroyed in reverse order of construction. p2718-exits-cxx20-constexpr.cpp
// is the C++20 twin, where the loop reads a destroyed temporary.
//@ expect: compile-ok
//@ strict-diags
//@ std: c++23
//@ xfail(clang): clang 19's and clang 22's constant evaluators destroy these temporaries in order of construction (their generated code does not)
struct Log {
  int v[40] = {};
  int n = 0;
  constexpr void add(int x) { v[n++] = x; }
  constexpr bool is(const int* e, int k) const {
    for (int i = 0; i < k; ++i)
      if (v[i] != e[i]) return false;
    return n == k;
  }
};
struct T {
  Log* l; int id;
  constexpr T(Log* l, int id) : l(l), id(id) { l->add(id); }
  constexpr ~T() { l->add(-id); }
};
struct L {
  Log* l; int a[2] = {1, 2};
  constexpr ~L() { l->add(-9); }
  constexpr const int* begin() const { return a; }
  constexpr const int* end() const { return a + 2; }
};
struct W {
  L x;
  constexpr const L& items() const& { return x; }
};
constexpr W wrap(const T& t) { return W{{t.l}}; }

constexpr int loop(Log& l, int stop) {
  for (int x : wrap(T(&l, 7)).items()) {
    l.add(x);
    if (x == stop) return x;
    if (x == -stop) break;
  }
  return 0;
}
constexpr bool exits() {
  Log l;
  loop(l, 0);
  loop(l, 1);
  loop(l, -2);
  const int e[] = {7, 1, 2, -9, -7, 7, 1, -9, -7, 7, 1, 2, -9, -7};
  return l.is(e, 14);
}
static_assert(exits());
