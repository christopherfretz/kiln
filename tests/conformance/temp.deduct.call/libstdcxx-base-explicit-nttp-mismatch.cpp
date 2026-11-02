// [temp.deduct.call]/4.3, [temp.deduct]/2: with the non-type parameter I
// given explicitly, P = Impl<I, H, T...> is Impl<1, H, T...> against the
// bases of A; the base Impl<0, ...> mismatches on the value and the next
// base Impl<1, ...> is the deduced one. libstdc++ tuple:2446
// (std::get<__i> calling std::__get_helper<__i>(tuple&)); <filesystem>,
// <mutex> scoped_lock (docs/status/libstdcxx.md, WP-332).
// Verified against clang 19.
//@ expect: run
//@ strict-diags
//@ stdout: 5 7 3 7
#include <stdio.h>
template<unsigned long I, class... T> struct Impl;
template<unsigned long I, class H, class... T>
struct Impl<I, H, T...> : Impl<I + 1, T...> { H h; };
template<unsigned long I, class H> struct Impl<I, H> { H h; };
template<class... T> struct Tup : Impl<0, T...> {};

template<unsigned long I, class H, class... T>
H &get(Impl<I, H, T...> &t) { return t.h; }
template<unsigned long I, class H, class... T>
const H &get(const Impl<I, H, T...> &t) { return t.h; }

// A written constant in P is compared the same way.
template<class H, class... T> H &second(Impl<1, H, T...> &t) { return t.h; }

int main() {
  Tup<int, long, char> t;
  get<0>(t) = 5;
  get<1>(t) = 7;
  get<2>(t) = 3;
  second(t) += 0;
  const Tup<int, long, char> &c = t;
  printf("%d %ld %d %ld\n", get<0>(c), get<1>(c), (int)get<2>(c), second(t));
}
