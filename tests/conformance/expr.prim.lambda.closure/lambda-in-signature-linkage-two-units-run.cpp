// [expr.prim.lambda.closure]/2, [basic.def.odr]/15, [basic.link]/4.4: a
// closure type in a function template's signature (return type,
// parameter type, NTTP type, a class template member's return type, a
// namespace's default template argument) has the linkage of the
// template, so two units that form the same specializations share one
// definition of each: the specializations, the closures' call operators
// and their static locals. Before WP-1077 kiln gave these closures, and
// the specializations naming them, internal linkage (spec/abi.md §8.15),
// so each unit had its own copies and every comparison below was 0.
//@ std: c++20
//@ expect: run
//@ args: %{test_dir}/Inputs/lambda-in-signature-linkage-other.cpp
//@ stdout: 1 1 1 1 1 1 1 / 1 1
#include <cstdio>
#include "Inputs/lambda-in-signature-linkage.h"
#define ADDR(e) (void *)(e),
extern void *other_statics[];
extern void (*other_ret)();
extern void (*other_nret)();
int main() {
  void *mine[] = {USES(ADDR)};
  for (unsigned i = 0; i < sizeof mine / sizeof *mine; ++i)
    std::printf("%d ", mine[i] == other_statics[i]);
  std::printf("/ %d %d\n", reinterpret_cast<void (*)()>(&ret<int>) == other_ret,
              reinterpret_cast<void (*)()>(&S<int>::nret) == other_nret);
}
