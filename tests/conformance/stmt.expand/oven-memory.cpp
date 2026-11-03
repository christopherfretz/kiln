// [stmt.expand]/5 in the Oven: each element is a scope of its own, so an
// element variable (and the storage its constructor allocates) is
// destroyed at the end of its element: an expansion over 48 elements that
// each allocate peaks at one element's allocation, not 48.
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang>=23
//@ oven: 36 peak_bytes<=512
//@ oven: 37 peak_bytes<=512
#include <array>
#include <utility>

struct S {
  int v;
  int* p = nullptr;
  constexpr S(int v) : v(v) {}
  constexpr S(const S& o) : v(o.v), p(new int[64]{}) {}
  constexpr ~S() { delete[] p; }
};

template <std::size_t N> constexpr std::array<S, N> make() {
  return [&]<std::size_t... I>(std::index_sequence<I...>) {
    return std::array<S, N>{S(int(I))...};
  }(std::make_index_sequence<N>{});
}

template <std::size_t N> constexpr int run() {
  static constexpr std::array<S, N> a = make<N>();
  int s = 0;
  template for (auto x : a) {
    x.p[0] = x.v;
    s += x.p[0];
  }
  return s;
}
static_assert(run<3>() == 0 + 1 + 2);
static_assert(run<48>() == 47 * 48 / 2);
