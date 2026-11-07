//@ expect: run
//@ strict-diags
//@ xfail(kiln): WP-799 follow-up, a mode attribute on a declaration of dependent type is dropped instead of applied at instantiation
//@ stdout: 8 8 16
// GCC mode on a member typedef or variable of a dependent type (clang
// `handleModeAttr` keeps the attribute and `instantiateDependentModeAttr`
// applies it to the instantiated declaration): the instantiation's type
// is the mode's.
#include <cstdio>

template <class T> struct W {
  typedef T t __attribute__((mode(DI)));
  static T v __attribute__((mode(TI)));
};
template <class T> T W<T>::v __attribute__((mode(TI)));

int main() {
  std::printf("%zu %zu %zu\n", sizeof(typename W<unsigned>::t), sizeof(W<int>::t),
              sizeof(W<int>::v));
}
