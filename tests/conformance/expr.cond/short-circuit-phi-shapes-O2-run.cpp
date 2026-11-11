// [expr.cond], [expr.log.and], [expr.log.or]: short-circuit-phi-shapes-run.cpp
// at -O2 (WP-1288).
//@ expect: run
//@ args: -O2
//@ stdout: 1 0 1 0 | 1 -1 3 -2 | 1 1 | 4 1 2 3 | 1 0 5 | 6 0 1
#include "short-circuit-phi-shapes-run.cpp"
