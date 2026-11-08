//@ args: -stdlib=libc++ -fconstexpr-steps=100000000
// The inplace_vector pattern in union storage ([class.union.general],
// [basic.life], P3074R7/P3726R2; spec/oven.md "C++26 union lifetime",
// WP-743, WP-760): a fixed-capacity vector whose elements live in
// `union { S data[1024]; }`. 100000 elements that own heap memory are
// pushed (std::construct_at activates the array member element by
// element); at each full buffer every element is read and popped
// (std::destroy_at). Every access to an element checks the union's active
// member. `oven/inplace-alloc` is the same workload with std::allocator
// storage: the two should take about the same time (WP-760 target: within
// 1.15x). kiln only: clang 19/22 lack P3074/P3726 (an element of an
// inactive array member cannot be constructed).
#include <memory>

struct S {
  int *p;
  constexpr S(int v) : p(new int(v)) {}
  S(const S &) = delete;
  constexpr ~S() { delete p; }
};

template <class T, int N> struct IV {
  union { T data[N]; };
  int n = 0;
  constexpr IV() {}
  constexpr ~IV() {
    for (int i = 0; i < n; ++i) std::destroy_at(&data[i]);
  }
  constexpr bool full() const { return n == N; }
  constexpr void push(int x) {
    std::construct_at(&data[n], x);
    ++n;
  }
  constexpr long long drain() {
    long long s = 0;
    while (n > 0) {
      --n;
      s += *data[n].p;
      std::destroy_at(&data[n]);
    }
    return s;
  }
};

constexpr long long run(int total) {
  IV<S, 1024> v;
  long long sum = 0;
  for (int i = 0; i < total; ++i) {
    if (v.full()) sum += v.drain();
    v.push(i);
  }
  return sum + v.drain();
}

static_assert(run(100000) == 100000LL * 99999 / 2);
