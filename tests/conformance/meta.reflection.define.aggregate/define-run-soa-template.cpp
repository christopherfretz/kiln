//@ expect: run
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ stdout: 48 18
//@ xfail(gcc): GCC 16.1 rejects a constexpr local's array member as a constexpr expansion statement's range
// P2996R13 §3.11 (struct of arrays), in the paper's shape: a class
// template's consteval block iterates the members of T with an expansion
// statement and defines the nested `impl` with an array of N of each.
#include "kiln_meta.h"
extern "C" int printf(const char*, ...);
using namespace std::meta;

template <class T, unsigned long N>
struct struct_of_arrays_impl {
  struct impl;
  consteval {
    constexpr auto members = __kiln_data_members<^^T>();
    info specs[members.size()];
    unsigned long n = 0;
    template for (constexpr info m : members.items) {
      specs[n++] = data_member_spec(^^typename[:type_of(m):][N], {.name = identifier_of(m)});
    }
    define_aggregate(^^impl, specs);
  }
};
template <class T, unsigned long N>
using struct_of_arrays = struct_of_arrays_impl<T, N>::impl;

struct point {
  float x;
  float y;
  float z;
};
struct_of_arrays<point, 4> soa;

int main() {
  for (int i = 0; i < 4; ++i) {
    soa.x[i] = float(i);
    soa.y[i] = float(2 * i);
    soa.z[i] = float(3 * i);
  }
  printf("%zu %g\n", sizeof(soa), soa.x[3] + soa.y[3] + soa.z[3]);
  return 0;
}
