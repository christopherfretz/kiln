// [stmt.pre]/4, [dcl.struct.bind] (C++26, P0963R3): a structured binding
// declaration may be the condition of `if`, `while` and `for`. The
// condition's value is that of the underlying (holding) object,
// contextually converted to bool, and is evaluated before the bindings are
// initialized ([stmt.pre]/4: so a tuple-like `get` is called only after
// the test). No diagnostic in C++26.
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
  constexpr explicit operator bool() const { return x != 0; }
};

// Tuple-like: `get` records that it ran.
int gets = 0;
struct T {
  int v;
  explicit operator bool() const { return gets == 0 && v > 0; }
  template <unsigned long I> int get() const { ++gets; return v + int(I); }
};
template <> struct std::tuple_size<T> { static constexpr unsigned long value = 2; };
template <unsigned long I> struct std::tuple_element<I, T> { using type = int; };

constexpr int first(P p) {
  if (auto [a, b] = p) return a + b;
  return -1;
}
static_assert(first(P{1, 2}) == 3);
static_assert(first(P{0, 2}) == -1);

constexpr int loop(int n) {
  int count = 0;
  P p{n, 0};
  while (auto [a, b] = p) { ++count; p.x = a - 1; }
  for (P q{n, 0}; auto [a, b] = q; q.x = a - 1) ++count;
  return count;
}
static_assert(loop(3) == 6);

// In a template, with a dependent type.
template <class U> constexpr int sum(U u) {
  if (auto [a, b] = u) return a + b;
  else return a - b;
}
static_assert(sum(P{3, 1}) == 4 && sum(P{0, 1}) == -1);

// Order in constant evaluation: `get` counts through a pointer; the test
// sees no `get` yet, both `get`s run whatever the outcome.
struct C {
  int* n;
  int v;
  constexpr explicit operator bool() const { return *n == 0 && v > 0; }
  template <unsigned long I> constexpr int get() const { ++*n; return v + int(I); }
};
template <> struct std::tuple_size<C> { static constexpr unsigned long value = 2; };
template <unsigned long I> struct std::tuple_element<I, C> { using type = int; };

constexpr int order(int v) {
  int n = 0;
  if (auto [a, b] = C{&n, v}) return n * 100 + a + b;
  else return -n;
}
static_assert(order(4) == 209 && order(0) == -2);

constexpr int order_loop(int k) {
  int n = 0, total = 0;
  while (auto [a, b] = C{&n, k}) { total += n + a; --k; n = 0; }
  int w = n;
  n = 0;
  for (int j = 2; auto [a, b] = C{&n, j}; --j) { total += n + b; n = 0; }
  return total * 100 + w * 10 + n;
}
static_assert(order_loop(3) == 2122);

int main() {
  int four = 4, zero = 0, three = 3;
  if (order(four) != 209 || order(zero) != -2 || order_loop(three) != 2122) return 3;
  if (auto [a, b] = T{4}) {
    if (gets != 2 || a != 4 || b != 5) return 1;
  } else {
    return 2;
  }
  return 0;
}
