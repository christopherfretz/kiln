// [temp.deduct.type]/5.3: a non-type template argument pack expansion whose pattern is an
// expression other than a bare template parameter (`conv<Args>()...`, `sizeof(Args)...`)
// is a non-deduced context, so the parameter takes no part in deduction and its argument
// converts to the substituted type ([temp.deduct.call]/1, /10); a bare `N...` is still
// deduced ([temp.deduct.type]/8).
//@ expect: run
//@ stdout: conv 1 4
//@ stdout: sizeof 2 5
//@ stdout: deduced 3 6
//@ stdout: by-value 2 3
#include <cstdio>

template <class T> consteval int conv() { return sizeof(T); }

template <int... C> struct Spec {
  constexpr Spec() {}
  consteval Spec(const char*) {}
  static constexpr int count = sizeof...(C);
  static constexpr int sum = (0 + ... + C);
};

template <class... Args> void fmt(const Spec<conv<Args>()...>&, const Args&... args) {
  std::printf("conv %d %d\n", int(sizeof...(args)), Spec<conv<Args>()...>::sum);
}

template <class... Args> void szf(Spec<int(sizeof(Args))...>, Args... args) {
  std::printf("sizeof %d %d\n", int(sizeof...(args)), Spec<int(sizeof(Args))...>::sum);
}

template <int... N> void ded(const Spec<N...>&) {
  std::printf("deduced %d %d\n", int(sizeof...(N)), (0 + ... + N));
}

template <int... N> void byval(Spec<N...>) {
  std::printf("by-value %d %d\n", int(sizeof...(N)), (0 + ... + N));
}

int main() {
  fmt("%08x", 1u);
  szf("%c %d", 'c', 1);
  ded(Spec<1, 2, 3>());
  byval(Spec<1, 2>());
}
