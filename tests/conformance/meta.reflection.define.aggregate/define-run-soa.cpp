//@ expect: run
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ stdout: 48 0 16 32
//@ stdout: 18 3.5
// P2996R13 §3.11 (struct of arrays), reduced to what kiln implements: a
// consteval block walks the members of `point` and defines `points` with
// one array member per member, named alike; the program fills and reads
// it at run time. (The paper's form, over a template and std::vector,
// is define-run-soa-template.cpp.)
#include "kiln_meta.h"
extern "C" int printf(const char*, ...);
using namespace std::meta;

struct point {
  float x;
  float y;
  float z;
};
struct points;
consteval {
  constexpr auto ctx = access_context::unchecked();
  info specs[nonstatic_data_members_of(^^point, ctx).size()];
  unsigned long i = 0;
  for (info m : nonstatic_data_members_of(^^point, ctx))
    specs[i++] = data_member_spec(^^float[4], {.name = identifier_of(m)});
  define_aggregate(^^points, specs);
}
points soa;

int main(int argc, char**) {
  for (int i = 0; i < 4; ++i) {
    soa.x[i] = float(i * argc);
    soa.y[i] = float(2 * i);
    soa.z[i] = float(3 * i);
  }
  soa.z[1] += 0.5f;
  printf("%zu %zu %zu %zu\n", sizeof(points), __builtin_offsetof(points, x),
         __builtin_offsetof(points, y), __builtin_offsetof(points, z));
  printf("%g %g\n", soa.x[3] + soa.y[3] + soa.z[3], soa.z[1]);
  return 0;
}
