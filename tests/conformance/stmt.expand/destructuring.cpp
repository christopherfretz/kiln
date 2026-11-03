// [stmt.expand]/5.3: an expansion-initializer that is neither a list nor
// an iterable range is decomposed as by a structured binding declaration
// `auto&& [u0, ..., uN-1] = init;` (constexpr for a constexpr
// for-range-declaration), element i being u_i: a tuple-like type, an
// aggregate whose members are in a base class, and an array (an array is
// never iterating, [stmt.expand]/3).
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang>=23
#include <tuple>

constexpr long tuple_sum() {
  std::tuple<int, long, char> t{1, 2, 3};
  long s = 0;
  template for (auto x : t) { s += x; }
  return s;
}
static_assert(tuple_sum() == 6);

// constexpr: over a prvalue and over a local constexpr tuple.
constexpr unsigned long tuple_sizes() {
  unsigned long n = 0;
  template for (constexpr auto x : std::tuple{1, 2L, 'c'}) {
    static_assert(x > 0);
    n += sizeof(x);
  }
  constexpr std::tuple<short, char> t{1, 2};
  template for (constexpr auto x : t) { n += sizeof(x) + x; }
  return n;
}
static_assert(tuple_sizes() == sizeof(int) + sizeof(long) + 1 + sizeof(short) + 1 + 1 + 2);

struct Base { int a; long b; };
struct Derived : Base {};
constexpr long base_sum() {
  long s = 0;
  template for (auto x : Derived{{10, 20}}) { s += x; }
  return s;
}
static_assert(base_sum() == 30);

constexpr int array_sum() {
  int a[3] = {1, 2, 3};
  int s = 0;
  template for (auto x : a) { s += x; }
  return s;
}
static_assert(array_sum() == 6);

// The bindings refer to the decomposed object: `auto&` writes through.
constexpr int write_through() {
  int a[2] = {1, 2};
  template for (auto& x : a) { x *= 10; }
  std::tuple<int, int> t{3, 4};
  template for (auto& x : t) { x += 1; }
  return a[0] + a[1] + std::get<0>(t) + std::get<1>(t);
}
static_assert(write_through() == 10 + 20 + 4 + 5);
