// [temp.dep.type] P2996R13: a splice-scope-specifier whose operand is
// value-dependent makes its nested-name-specifier dependent; qualified
// lookup into the designated namespace, class or enumeration happens at
// instantiation, as do `typename [: R :]::type` and a splice of a type
// template parameter's reflection. Run through codegen.
//@ expect: run
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ stdout: 3 5 4 6
#include "kiln_meta.h"
extern "C" int printf(const char*, ...);
namespace N { int z = 3; }
struct C {
  static constexpr int z = 5;
  using type = int;
};
enum class E { z = 6 };
template <std::meta::info R> int z() { return static_cast<int>([: R :]::z); }
template <std::meta::info R> int t() { typename [: R :]::type v = 4; return v; }
template <class T> int dz() { return static_cast<int>([: ^^T :]::z); }

int main() {
  printf("%d %d %d %d\n", z<^^N>(), z<^^C>(), t<^^C>(), dz<E>());
  return 0;
}
