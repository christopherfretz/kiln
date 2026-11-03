//@ expect: run
//@ requires: clang>=22
//@ strict-diags
//@ stdout: 19 21 15 7 30 8
//@ stdout: 1 z 2 20
//@ stdout: 17
// [dcl.struct.bind] (C++26, P2686R5): constexpr structured bindings at run
// time. A binding used as a constant (an array bound, a template
// argument) needs no storage; an odr-use (its address, a lambda capture,
// a std::get of the referenced tuple) reads the holding variable, which
// codegen still emits, as for a static constexpr tuple-like declaration
// whose get results are constant-initialized hidden variables.
#include <tuple>
#include <array>
#include <utility>
extern "C" int printf(const char*, ...);
struct S { int a, b; };
template <class T> constexpr int tf(T t) {
  constexpr auto [x, y] = T{3, 4};
  static_assert(x == 3);
  static constexpr auto [p, q] = std::pair{5, 6};
  static_assert(p == 5);
  return x + y + p + q + t.a;
}
static_assert(tf(S{1, 2}) == 19);
template <class T> int packs() {
  static constexpr auto [...xs] = std::tuple{1, 2L, 3u};
  static_assert(sizeof...(xs) == 3);
  static_assert((xs + ...) == 6);
  constexpr auto [h, ...ts] = std::array{4, 5, 6};
  return (h + ... + ts) + (xs + ...);
}
constexpr std::tuple<int, double, char> gt{1, 2.5, 'z'};
constexpr auto& [r0, r1, r2] = gt;
static_assert(r0 == 1 && r1 == 2.5 && r2 == 'z');
constexpr auto [c0, c1, c2] = gt;
static_assert(&c0 != &r0 && c2 == 'z');
template <int N> struct Tag { static constexpr int v = N; };
int use_ptr(const int* p) { return *p; }
int main() {
  constexpr auto [a, b] = S{7, 8};
  int arr[Tag<a>::v];
  static constexpr auto [t0, t1] = std::tuple{10, 20};
  static_assert(t1 == 20);
  printf("%d %d %d %d %d %d\n", tf(S{1, 2}), packs<int>(), a + b, (int)sizeof(arr) / 4, t0 + t1, use_ptr(&b));
  printf("%d %c %d %d\n", r0, r2, c0 + use_ptr(&std::get<0>(gt)), use_ptr(&t1));
  auto lam = [=] { return a + t0; };
  printf("%d\n", lam());
}
