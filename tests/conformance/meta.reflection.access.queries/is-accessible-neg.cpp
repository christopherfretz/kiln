// [meta.reflection.access.queries] (P2996R13): is_accessible needs a
// designating class (the context's, or the member's own) that is a
// complete class, so a member of an incomplete or not-yet-complete class
// is an error; the has_inaccessible_* queries need a complete class
// type. Each failure throws std::meta::exception.
//@ expect: compile-fail
//@ args: -freflection -I tests/support
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ diag: 33:15 error "static assertion expression is not an integral constant expression"
//@ diag: 33:16 note "not a complete class type"
//@ diag: 34:15 error "static assertion expression is not an integral constant expression"
//@ diag: 34:16 note "not a complete class type"
//@ diag: 35:15 error "static assertion expression is not an integral constant expression"
//@ diag: 35:16 note "not a complete class type"
//@ diag: 36:15 error "static assertion expression is not an integral constant expression"
//@ diag: 36:16 note "not a complete class type"
//@ diag: 38:15 error "static assertion expression is not an integral constant expression"
//@ diag: 38:16 note "not a complete class type"
//@ diag: 41:29 error "call to consteval function 'std::meta::is_accessible' is not a constant expression"
//@ diag: 41:29 note "incomplete parent class"
//@ diag: 41:25 error "constexpr variable 'b' must be initialized by a constant expression"
//@ diag: 41:29 note "incomplete parent class"
#ifdef __KILN__
#include "kiln_meta.h"
#else
#include <meta>
#endif
using namespace std::meta;
struct Inc;
constexpr auto here = access_context::current();
static_assert(!has_inaccessible_bases(^^Inc, here));
static_assert(!has_inaccessible_nonstatic_data_members(^^Inc, here));
static_assert(!has_inaccessible_subobjects(^^Inc, here));
static_assert(!has_inaccessible_bases(^^int, here));
namespace N {}
static_assert(!has_inaccessible_subobjects(^^N, here));
struct Late {
  int m;
  static constexpr bool b = is_accessible(^^m, access_context::unprivileged());
};
