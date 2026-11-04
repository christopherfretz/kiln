// [temp.deduct.type]/5.3, [temp.arg.explicit]/4: with no trailing arguments the pack
// deduced only from the non-deduced `Spec<conv<Args>()...>` is empty, so the parameter is
// `const Spec<>&` and the literal converts through its consteval constructor.
//@ expect: run
//@ stdout: 0 0
//@ stdout: 0 0
#include <cstdio>

template <class T> consteval int conv() { return sizeof(T); }

template <int... C> struct Spec {
  consteval Spec(const char*) {}
  static constexpr int count = sizeof...(C);
};

template <class... Args> using FS = Spec<conv<Args>()...>;

template <class... Args> void fmt(const FS<Args...>&, const Args&... args) {
  std::printf("%d %d\n", int(sizeof...(args)), FS<Args...>::count);
}

template <class... Args> void only(const Spec<conv<Args>()...>&) {
  std::printf("%d %d\n", int(sizeof...(Args)), Spec<conv<Args>()...>::count);
}

int main() {
  fmt("x");
  only("y");
}
