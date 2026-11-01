//@ expect: no-crash
//@ diag: 5:9 error "undeclared identifier"
// Errors anywhere (here and in an included header) are fine: only a crash,
// a hang or an internal-error exit code fails a no-crash test.
int x = nosuch;
#include "does-not-exist.h"
