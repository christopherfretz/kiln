// [stmt.pre]/4, [stmt.switch]/2 (C++26, P0963R3): a structured binding
// declaration as the condition of a `switch`. The underlying object is
// converted once to the switch's integral type by its conversion function
// ([conv.general]/5), before a tuple-like `get` initializes the bindings;
// the bindings are in scope for the whole switch body.
//@ expect: run
//@ std: c++26
//@ strict-diags
//@ requires: clang>=22
namespace std {
template <class T> struct tuple_size;
template <unsigned long I, class T> struct tuple_element;
}

struct P {
  int x, y;
  constexpr operator int() const { return x + y; }
};

constexpr int agg(P p) {
  switch (auto [a, b] = p) {
  case 3: return a * 100 + b;
  case 0: return -2;
  default: return -1;
  }
}
static_assert(agg(P{1, 2}) == 102 && agg(P{0, 0}) == -2 && agg(P{4, 4}) == -1);

// Tuple-like: the conversion sees no `get` yet, both `get`s run whatever
// the selected case.
struct C {
  int* n;
  int v;
  constexpr operator int() const { return *n * 10 + v; }
  template <unsigned long I> constexpr int get() const { ++*n; return v + int(I); }
};
template <> struct std::tuple_size<C> { static constexpr unsigned long value = 2; };
template <unsigned long I> struct std::tuple_element<I, C> { using type = int; };

constexpr int order(int v) {
  int n = 0;
  switch (auto [a, b] = C{&n, v}) {
  case 1: return n * 100 + a + b;
  default: return -n;
  }
}
static_assert(order(1) == 203 && order(5) == -2);

// In a template, with a dependent type.
template <class U> constexpr int dep(U u) {
  switch (auto [a, b] = u) {
  case 3: return a - b;
  default: return 0;
  }
}
static_assert(dep(P{2, 1}) == 1 && dep(P{5, 5}) == 0);

int main() {
  int one = 1, five = 5;
  if (order(one) != 203 || order(five) != -2) return 1;
  P p{1, 2};
  if (agg(p) != 102 || dep(p) != -1) return 2;
  return 0;
}
