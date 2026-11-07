//@ expect: run
//@ std: c++20
//@ stdout: 5 3 7 3 9 11 4 2 3
// [dcl.fct.default]/4: a function parameter pack after a parameter
// with a default argument needs none. [temp.inst]/12: a default
// argument is instantiated only when a call uses it, also when a pack
// expanded (here, to nothing) shifts the parameters.
#include <cstdio>

struct NoDefault {
  NoDefault(int v) : v(v) {}
  int v;
};

template <typename C, typename... A>
int f(C c = C{}, A &&...a) {
  return c.v + static_cast<int>(sizeof...(a));
}

// A lambda with a capture has no default constructor.
template <typename C, typename... A>
int k(C c = C{}, A &&...a) {
  return c(1, 2) + static_cast<int>(sizeof...(a));
}

template <typename C = int, typename... A>
int g(int x = 7, A... a) {
  return x + static_cast<int>(sizeof...(a));
}

template <typename... A>
int h(A... a, int x = 9) {
  return x + static_cast<int>(sizeof...(a));
}

int main() {
  int base = 1;
  auto cmp = [base](int a, int b) { return base + a + b - 1; };
  auto l = [](int x = 2, auto... a) { return x + static_cast<int>(sizeof...(a)); };
  std::printf("%d %d %d %d %d %d %d %d %d\n", f(NoDefault(5)), k(cmp), g(), g(3), h(),
              h<int, int>(0, 0), h<int>(1, 3), l(), l(1, 'a', 2.0));
}
