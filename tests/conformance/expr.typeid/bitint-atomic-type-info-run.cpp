// [expr.typeid]/1, [except.handle]: typeid, throw and handlers of a
// _BitInt or an _Atomic type (and pointers to them) yield a std::type_info
// the program emits itself (an abi::__fundamental_type_info, which the C++
// runtime does not provide; clang BuildTypeInfo). kiln failed ("RTTI for
// type ... is not supported yet").
//@ expect: run
//@ stdout: DB7_ DU65_ U7_Atomici PU7_Atomici PKDB7_ 1 0
//@ stdout: caught 9
//@ stdout: caught atomic
#include <cstdio>
#include <typeinfo>

template <class T> const std::type_info &ti() { return typeid(T); }

int main() {
  std::printf("%s %s %s %s %s %d %d\n", typeid(_BitInt(7)).name(),
              typeid(unsigned _BitInt(65)).name(), ti<_Atomic(int)>().name(),
              typeid(_Atomic(int) *).name(), typeid(const _BitInt(7) *).name(),
              typeid(_BitInt(7)) == ti<_BitInt(7)>(),
              typeid(_BitInt(7)) == typeid(_BitInt(8)));
  try {
    throw (_BitInt(9))9;
  } catch (_BitInt(8)) {
    std::printf("wrong\n");
  } catch (_BitInt(9) v) {
    std::printf("caught %d\n", (int)v);
  }
  static _Atomic(int) a;
  try {
    throw &a;
  } catch (_Atomic(int) *p) {
    std::printf("caught %s\n", p == &a ? "atomic" : "other");
  }
}
