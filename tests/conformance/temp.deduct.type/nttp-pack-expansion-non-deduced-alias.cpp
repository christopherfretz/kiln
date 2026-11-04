// [temp.deduct.type]/5.3, [temp.deduct.call]/1: a parameter whose type is an alias for a
// specialization with a non-type pack expansion of consteval calls on the deduced pack
// (`Spec<conv<Args>()...>`) is a non-deduced context; `Args` comes from the trailing
// function parameter pack and the string literal then converts through the consteval
// converting constructor of the substituted type ([temp.deduct.call]/10, [expr.const]/6).
//@ expect: run
//@ stdout: 1 4 1
//@ stdout: 2 8 2
//@ stdout: 3 9 3
#include <cstdio>

template <class T> consteval int conv() { return sizeof(T); }

template <int... C> struct Spec {
  consteval Spec(const char*) {}
  static constexpr int count = sizeof...(C);
  static constexpr int sum = (0 + ... + C);
};

template <class... Args> using FS = Spec<conv<Args>()...>;

template <class... Args> void fmt(const FS<Args...>&, const Args&... args) {
  std::printf("%d %d %d\n", int(sizeof...(args)), FS<Args...>::sum, FS<Args...>::count);
}

int main() {
  unsigned x = 1;
  fmt("%08x", x);
  fmt("%d %d", x, 2);
  fmt("%d %c %d", x, 'c', 2u);
}
