// [namespace.udecl]/11, [temp.res.general]/1: a call in a template
// through a using-declaration sees only the overloads it introduced, not
// one declared later in the nominated namespace; a wrong assertion about
// the result still fails, as without a precompiled header (WP-1342).
//@ expect: compile-fail
//@ diag: 12:15 error "static assertion failed due to requirement"
//@ diag: 12:35 note "expression evaluates to '-1 == 100'"
#include "Inputs/using_shadow_calls.h"
namespace usc_lib {
constexpr short mag(short) { return 100; }
} // namespace usc_lib
static_assert(usc::alt(short(-1)) == 100);
