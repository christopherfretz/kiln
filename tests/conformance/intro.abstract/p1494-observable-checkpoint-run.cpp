// [intro.abstract]/5 (P1494R5): an observable checkpoint bounds the
// effect of undefined behaviour; the observable behaviour before it is
// kept. kiln provides GCC's `__builtin_observable_checkpoint()` (a
// `void` noexcept builtin, what libstdc++'s `std::observable_checkpoint`
// calls), reported by `__has_builtin`. As in GCC it is a no-op in
// constant evaluation (the library function itself is not constexpr).
// clang has no such builtin; GCC 16.1 agrees with every line here.
//@ skip(clang): clang does not provide __builtin_observable_checkpoint
//@ expect: run
//@ strict-diags
//@ stdout: 1 7 3
#include <cstdio>
static_assert(__has_builtin(__builtin_observable_checkpoint));
static_assert(noexcept(__builtin_observable_checkpoint()));
static_assert(__is_same(decltype(__builtin_observable_checkpoint()), void));
constexpr int c() { __builtin_observable_checkpoint(); return 3; }
static_assert(c() == 3);
int g;
void step(volatile int* v, int n) {
  g = n;
  __builtin_observable_checkpoint();
  *v = n;
}
int main() {
  volatile int v = 0;
  step(&v, 7);
  std::printf("%d %d %d\n", __has_builtin(__builtin_observable_checkpoint), g, c());
}
