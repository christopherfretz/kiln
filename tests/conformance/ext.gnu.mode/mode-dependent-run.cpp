//@ expect: run
//@ strict-diags
//@ stdout: 8 8 16 2 1 804 8
// GCC mode on a member typedef, variable or field of a dependent type
// (clang `handleModeAttr` keeps the attribute and
// `instantiateDependentModeAttr` applies it to the instantiated
// declaration): the instantiation's type is the mode's, wherever the
// attribute is written.
#include <cstdio>

template <class T> struct W {
  typedef T t __attribute__((mode(DI)));
  static T v __attribute__((mode(TI)));
};
template <class T> T W<T>::v __attribute__((mode(TI)));

template <class T> struct F { T x __attribute__((mode(HI))); };
template <class T> struct D { __attribute__((mode(QI))) T z; };

template <class T> int local() {
  T y __attribute__((mode(DI))) = 0;
  typedef T u __attribute__((mode(SI)));
  return sizeof(y) * 100 + sizeof(u);
}

int main() {
  std::printf("%zu %zu %zu %zu %zu %d %zu\n", sizeof(typename W<unsigned>::t), sizeof(W<int>::t),
              sizeof(W<int>::v), sizeof(F<long>), sizeof(D<int>), local<char>(),
              sizeof(D<int>) + sizeof(W<char>::t) - 1);
}
