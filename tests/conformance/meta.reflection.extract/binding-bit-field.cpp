// [meta.reflection.extract]/12: extract by value of a structured binding
// naming a bit-field of a decomposition whose lifetime began within the
// evaluation is `extract-value<U>(constant_of(r))`, and constant_of reads
// the bit-field through `[: r :]` ([meta.reflection.queries]): its value.
// No GCC oracle: GCC 16.1 rejects it ("value cannot be extracted"); the
// text decides. Reduced from real code.
//@ expect: compile-ok
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ strict-diags
//@ xfail(kiln): WP-2191 follow-up, a bit-field binding has no address the Oven keeps for its reflection
#include <meta>

struct BF { int x : 3; int y : 5; };

consteval int bit_field() {
  auto [x, y] = BF{1, 2};
  y = 3;
  return std::meta::extract<int>(^^y) * 10 + std::meta::extract<int>(std::meta::constant_of(^^x));
}
static_assert(bit_field() == 31);
