// [cpp.include], [cpp.cond]: a header with an #ifndef include guard can be
// included repeatedly; its macros expand in the including file.
//@ expect: compile-ok
//@ strict-diags
#include "Inputs/guarded.h"
#include "Inputs/guarded.h"
#define HEADER "Inputs/guarded.h"
#include HEADER
static_assert(GUARDED_SIZE == sizeof(Guarded));
static_assert(GUARDED_SIZE >= sizeof(long) + 1);
