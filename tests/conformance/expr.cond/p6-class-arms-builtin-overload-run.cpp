// [expr.cond]/6, [over.built]/25-26: arms of different types, one of class
// type, that /4 cannot convert to each other: overload resolution over the
// built-in `operator?:` candidates converts them. `L R` arithmetic
// candidates take the promoted types, so two `bool`-converting classes
// give `int`; `T(T, T)` covers pointers and scoped enumerations.
// Boost.Hana `eval_if` returns `c ? true_{} : false_{}`.
//@ expect: run
//@ stdout: 1 0 7 2.5 1 2 1
extern "C" int printf(const char*, ...);

template <class T, class U> constexpr bool same = false;
template <class T> constexpr bool same<T, T> = true;

template <bool B> struct ic {
  constexpr operator bool() const { return B; }
};
struct I { operator int() const { return 7; } };
struct P { int* p; operator int*() const { return p; } };
struct Q { const int* p; operator const int*() const { return p; } };
enum class E { a = 1, b = 2 };
struct EA { operator E() const { return E::a; } };
struct EB { operator E() const { return E::b; } };

constexpr int pick(bool c) { return c ? ic<true>{} : ic<false>{}; }
static_assert(pick(true) == 1 && pick(false) == 0);

int main(int argc, char**) {
  bool c = argc > 0;
  auto x = c ? ic<true>{} : ic<false>{};
  static_assert(same<decltype(x), int>);
  auto y = !c ? ic<true>{} : ic<false>{};
  auto z = c ? I{} : 2.5;
  static_assert(same<decltype(z), double>);
  auto w = !c ? I{} : 2.5;
  int n = 1;
  auto p = c ? P{&n} : Q{&n};
  static_assert(same<decltype(p), const int*>);
  auto e = !c ? EA{} : EB{};
  static_assert(same<decltype(e), E>);
  auto f = c ? EA{} : E::b;
  static_assert(same<decltype(f), E>);
  printf("%d %d %g %g %d %d %d\n", x, y, z, w, *p, int(e), int(f));
}
