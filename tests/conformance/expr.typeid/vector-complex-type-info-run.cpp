// [expr.typeid]/1, [support.rtti]: typeid of a GNU vector type or a
// _Complex type yields a std::type_info that the program emits itself
// (an abi::__fundamental_type_info, as for a builtin type, but not one the
// C++ runtime provides; clang `BuildTypeInfo`). kiln rejected it ("RTTI
// for type ... is not supported yet"; Eigen's packetmath test names its
// SIMD packet types with typeid).
//@ expect: run
//@ stdout: Dv4_f Dv4_i Cd 1 0
#include <cstdio>
#include <typeinfo>

typedef float v4f __attribute__((vector_size(16)));
typedef int v4i __attribute__((vector_size(16)));

template <class T> const std::type_info &ti() { return typeid(T); }

int main() {
  std::printf("%s %s %s %d %d\n", typeid(v4f).name(), ti<v4i>().name(),
              typeid(_Complex double).name(), typeid(v4f) == ti<v4f>(),
              typeid(v4f) == typeid(v4i));
}
