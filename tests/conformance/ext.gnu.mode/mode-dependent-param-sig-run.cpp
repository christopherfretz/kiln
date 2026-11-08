//@ expect: run
//@ strict-diags
//@ stdout: 8 8 4 8 8
// GCC mode on function parameters of dependent type (WP-809): clang
// `instantiateDependentModeAttr` applies it to each substituted parameter,
// a pack's elements included, and the instantiation's function type is
// built from those parameters.
#include <cstdio>

template <class T> int f(T x __attribute__((mode(DI)))) { return sizeof(x); }
template <class T> auto g(T x __attribute__((mode(DI)))) { return x; }
template <class... T> int h(T... x __attribute__((mode(HI)))) { return (0 + ... + sizeof(x)); }
template <class T> struct S {
  int m(T y __attribute__((mode(DI)))) { return sizeof(y); }
};

int (*p)(long) = &f<int>;
static_assert(__is_same(decltype(g<int>(0)), long));
static_assert(__is_same(decltype(f<int>), int(long)));

int main() {
  std::printf("%d %d %d %d %d\n", f<int>(1), (int)sizeof(g<int>(3)), h<int, int>(1, 2),
              S<int>().m(1), p(5));
}
