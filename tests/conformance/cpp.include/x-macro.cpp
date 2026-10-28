// [cpp.include], [cpp.replace]: the X-macro idiom; one included list is
// expanded with different definitions of X to build declarations.
//@ expect: compile-ok
//@ strict-diags
#define X(name, value) constexpr int name = value;
#include "Inputs/xmacro.def"
#undef X
#define X(name, value) + name
constexpr int total = 0
#include "Inputs/xmacro.def"
;
#undef X
static_assert(total == 7);
static_assert(red == 1 && green == 2 && blue == 4);
