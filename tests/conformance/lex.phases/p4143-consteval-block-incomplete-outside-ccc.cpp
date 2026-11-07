// [lex.phases]/1.7 as amended by P4143R0 (Croydon DR): the side effect
// of a consteval block's define_aggregate is complete only from points
// from which the block is reachable. A static data member initializer of
// a nested class is not a complete-class context of the enclosing class
// and precedes the block, so S::Incomplete is still incomplete there.
// GCC 16.1 gives the same error; clang does not implement P2996 (the
// complete half is p4143-consteval-block-complete-class-context-reflection.cpp).
//@ expect: compile-fail
//@ args: -freflection -I tests/support
//@ skip(clang): clang does not implement P2996 reflection
//@ strict-diags
//@ diag: 18:30 error "invalid application of 'sizeof' to an incomplete type 'Incomplete'"
//@ diag: 16:9 note "forward declaration of 'S::Incomplete'"
#include "kiln_meta.h"
class S {
  class Incomplete;
  class Inner {
    static constexpr int n = sizeof(Incomplete); // error: incomplete here
  };
  consteval {
    std::meta::define_aggregate(^^Incomplete, {});
  }
};
