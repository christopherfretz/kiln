// The `c` case of the pch bench group (spec/bench.md "The pch group"): the
// C library headers most C programs open with, plus zlib's interface, as a
// C PCH (`-x c-header`).
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <zlib.h>
