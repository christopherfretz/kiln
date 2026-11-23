// [temp.inst]/3, [dcl.attr.grammar]/5: parameter attributes kept across a
// precompiled header: a function template's parameter with a
// value-dependent `align_value(N)` and its `nonnull` and `annotate`
// instantiated in the consumer, and a prior declaration's `nonnull` and
// `annotate` inherited by the consumer's definition. spec/sema.md §4.5
// "Parameter attributes", WP-2476.
//@ expect: run
//@ pch: Inputs/param-attrs-pch.h
//@ stdout: 7
#include <cstdio>
int prior(int *p, int *q) { return *p * *q; }
int main() {
  alignas(16) int a = 2;
  int b = 1;
  std::printf("%d\n", load<16>(&a) + first(&a, &b) + prior(&a, &b) + load<4>(&b) - 1);
}
