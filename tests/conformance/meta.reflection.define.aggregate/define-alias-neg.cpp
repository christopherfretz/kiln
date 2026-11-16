// [meta.reflection.define.aggregate]/8: define_aggregate is a constant
// subexpression only when class_type represents a cv-unqualified class
// type. A reflection of a type alias represents the alias, not a class
// type, even when the alias names an incomplete class; the class itself
// is accepted. Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ args: -ferror-limit=0 -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ diag: 23:1 error "consteval block is not a constant expression"
//@ diag: 23:13 note "'^^Alias' is a type alias, not a class type"
//@ diag: 23:1 note "in call to"
#include "kiln_meta.h"
#include "kiln_meta_queries.h"

struct Plain;
consteval { std::meta::define_aggregate(^^Plain, {}); }
static_assert(std::meta::is_complete_type(^^Plain));

struct Aliased;
using Alias = Aliased;
consteval { std::meta::define_aggregate(^^Alias, {}); }
consteval { std::meta::define_aggregate(std::meta::dealias(^^Alias), {}); }
static_assert(std::meta::is_complete_type(^^Aliased));
