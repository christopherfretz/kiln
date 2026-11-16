// [stmt.expand]/5.2: the iterating form names begin-expr, end-expr,
// `b != e` and `++b` (the count), `begin - begin` (the index type),
// `begin + i` and `*iter`; an iterator with all of them is accepted. The
// member form needs both `begin` and `end` members ([stmt.ranged]/1.3.2),
// so a class with only one of them and no begin/end found by
// argument-dependent lookup is not expansion-iterable ([stmt.expand]/3) and
// is destructured instead (GCC 16.1 agrees).
//@ expect: compile-ok
//@ requires: clang>=23
//@ oracle: gcc
struct It {
  int i;
  constexpr int operator*() const { return i; }
  constexpr It &operator++() { ++i; return *this; }
  constexpr It operator+(long d) const { return {int(i + d)}; }
  constexpr long operator-(It o) const { return i - o.i; }
  constexpr bool operator!=(It o) const { return i != o.i; }
};
struct R {
  constexpr It begin() const { return {0}; }
  constexpr It end() const { return {3}; }
};
namespace n {
struct A { int v[2]; };
constexpr const int *begin(const A &a) { return a.v; }
constexpr const int *end(const A &a) { return a.v + 2; }
}
struct OnlyBegin { int a, b; int *begin(); };
struct OnlyEnd { int a, b; int *end(); };
constexpr int f() {
  int s = 0;
  template for (constexpr int x : R{}) { s += x; }
  template for (auto x : R{}) { s += x * 10; }
  template for (auto x : n::A{{4, 5}}) { s += x * 100; }
  template for (auto x : OnlyBegin{1, 2}) { s += x * 1000; }
  template for (auto x : OnlyEnd{1, 2}) { s += x * 10000; }
  return s;
}
static_assert(f() == 33933);
