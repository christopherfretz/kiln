// [class.temporary]/6.4 (P2718R0) during constant evaluation
// ([expr.const]): the destruction order of the for-range-initializer's
// temporaries is observable in a constexpr log. In C++23 `T(7)` outlives
// the loop; in C++20 it is destroyed before the first iteration.
//@ std: c++23
//@ xfail(clang): clang 19's constant evaluator destroys the extended temporaries in construction order, unlike its own codegen and [class.temporary]/8

struct Log {
  int v[8] = {};
  int n = 0;
  constexpr void add(int x) { v[n++] = x; }
};
struct T {
  Log* l;
  int id;
  constexpr T(Log* l, int i) : l(l), id(i) { l->add(i); }
  constexpr ~T() { l->add(-id); }
};
struct L {
  Log* l;
  int a[2] = {1, 2};
  constexpr ~L() { l->add(100); }
  constexpr const int* begin() const { return a; }
  constexpr const int* end() const { return a + 2; }
};
constexpr L make(Log* l, const T&) { return L{l}; }
constexpr const L& id(const L& l) { return l; }

constexpr bool order() {
  Log log;
  for (int x : id(make(&log, T(&log, 7)))) log.add(x * 10);
  constexpr int want[] = {7, 10, 20, 100, -7};
  if (log.n != 5) return false;
  for (int i = 0; i < 5; ++i)
    if (log.v[i] != want[i]) return false;
  return true;
}
static_assert(order());

constexpr int direct() {
  Log log;
  for (int x : make(&log, T(&log, 3))) log.add(x);
  return log.v[0] * 10000 + log.v[1] * 1000 + log.v[2] * 100 + log.v[3] - log.v[4];
}
static_assert(direct() == 3 * 10000 + 1 * 1000 + 2 * 100 + 100 + 3);
