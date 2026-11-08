//@ args: -stdlib=libc++ -fconstexpr-steps=100000000
// The inplace_vector workload of `oven/inplace-union` with std::allocator
// storage (spec/oven.md "Transient allocation", WP-743, WP-760): the
// comparison for union storage, which should take about the same time
// (WP-760 target: union within 1.15x of this entry). 100000 elements that
// own heap memory are pushed into a capacity-1024 buffer from
// std::allocator<S>::allocate (std::construct_at); at each full buffer
// every element is read and popped (std::destroy_at).
#include <memory>

struct S {
  int *p;
  constexpr S(int v) : p(new int(v)) {}
  S(const S &) = delete;
  constexpr ~S() { delete p; }
};

template <class T, int N> struct AV {
  std::allocator<T> a;
  T *data;
  int n = 0;
  constexpr AV() : data(a.allocate(N)) {}
  constexpr ~AV() {
    for (int i = 0; i < n; ++i) std::destroy_at(&data[i]);
    a.deallocate(data, N);
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
  AV<S, 1024> v;
  long long sum = 0;
  for (int i = 0; i < total; ++i) {
    if (v.full()) sum += v.drain();
    v.push(i);
  }
  return sum + v.drain();
}

static_assert(run(100000) == 100000LL * 99999 / 2);
