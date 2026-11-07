// [intro.abstract]/5, [utility.undefined] (P1494R5): libstdc++ 16.1's
// `<utility>` declares `std::observable_checkpoint()` (and
// `__cpp_lib_observable_checkpoint`) when `__has_builtin
// (__builtin_observable_checkpoint)`, which kiln provides. The library
// function is not constexpr.
//@ std: c++26
//@ expect: run
//@ strict-diags
//@ args: --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not provide __builtin_observable_checkpoint
//@ oracle: gcc
//@ stdout: 202506 1
#include <cstdio>
#include <utility>
static_assert(noexcept(std::observable_checkpoint()));
int main() {
  int x = 0;
  x = 1;
  std::observable_checkpoint();
  std::printf("%ld %d\n", long(__cpp_lib_observable_checkpoint), x);
}
