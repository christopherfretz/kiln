// spec/reflection.md §8.2: a speculative evaluation (here
// __builtin_constant_p's operand) does not throw std::meta::exception; a
// failing reflection function there is simply not constant, and an
// enclosing handler sees nothing.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
//@ xfail(gcc): GCC 16.1 throws std::meta::exception out of __builtin_constant_p's operand (spec/reflection.md §8.2)
#include "kiln_meta.h"
using namespace std::meta;

consteval int probe() {
  try {
    return __builtin_constant_p(identifier_of(^^int).size()) ? 1 : 2;
  } catch (...) {
    return 3;
  }
}
static_assert(probe() == 2);
static_assert(__builtin_constant_p(identifier_of(^^decltype(probe)).size()) == 0);

int main() {}
