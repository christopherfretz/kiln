// C17 7.25 (type-generic math) at -std=gnu89: clang's <tgmath.h>
// declares `__tg_promote(...)` with only `...`, which C before C23
// allows on an `overloadable` function (WP-1331).
//@ args: -std=gnu89 -lm
//@ expect: run
//@ stdout: f d ld cd
//@ stdout: f d ld cd
//@ stdout: f d ld f
//@ stdout: f d d
//@ stdout: 2 3 4 2
//@ stdout: 16 9 2.23607 3
#include "Inputs/tgmath-calls.h"
int main(void) { return tgmath_calls(); }
