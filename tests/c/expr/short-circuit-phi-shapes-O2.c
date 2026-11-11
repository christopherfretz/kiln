// short-circuit-phi-shapes.c at -O2 (C23 6.5.14, 6.5.15; WP-1288).
//@ std: c17
//@ expect: run
//@ args: -O2
//@ stdout: 1 0 1 0 | 1 1 | 4 1 2 3 | 1 0 5 | 6 0 1 | 2 3
#include "short-circuit-phi-shapes.c"
