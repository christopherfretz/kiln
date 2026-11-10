// [class.temporary]/7 (C++23, P2718R0), [intro.execution]/5,
// [class.temporary]/6 (CWG1815), during constant evaluation ([expr.const]):
// a default member initializer's temporary used in a for-range-initializer
// outlives the loop, the destruction order observable in a constexpr log;
// outside one it ends with the full-expression; and a reference member's
// temporary from its default member initializer is extended with a
// constexpr variable, so reading it is a constant expression. The Oven
// evaluates and destroys the use's rebuilt copy (spec/sema.md §7.6a).
// clang 22 agrees; clang 19 extends neither, and GCC 16.1 rejects the loop.
// WP-1177.
//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
//@ requires: clang>=22
struct T {
  int* log;
  int id;
  constexpr ~T() { *log = *log * 10 + id; }
};
struct Agg {
  int* log;
  int x = T{log, 4}.id;
};
struct Outer {
  Agg in;
  int y = T{in.log, 5}.id;
};
struct Rng {
  int a[2] = {1, 2};
  constexpr const int* begin() const { return a; }
  constexpr const int* end() const { return a + 2; }
};
template <class G> constexpr Rng mk(const G&) { return Rng{}; }

constexpr int loop() {
  int log = 0;
  for (int v : mk(Agg{&log}))
    log = log * 10 + v;
  return log;
}
static_assert(loop() == 124);

constexpr int nested() {
  int log = 0;
  for (int v : mk(Outer{{&log}}))
    log = log * 10 + v;
  return log;
}
static_assert(nested() == 1254);

constexpr int decl() {
  int log = 0;
  {
    Agg a{&log};
    log = log * 10 + 9;
  }
  return log;
}
static_assert(decl() == 49);

struct CR {
  const int& r = 5;
  int y = 2;
};
constexpr CR c{};
static_assert(c.r == 5 && c.y == 2);
