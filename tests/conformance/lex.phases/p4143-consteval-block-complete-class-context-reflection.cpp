// [lex.phases]/1.7 as amended by P4143R0 (Croydon DR, "Constant
// evaluation fixes"): each manifestly constant-evaluated expression is
// evaluated during translation, and its side effects are complete from
// any point from which the evaluation is reachable. The paper's example:
// the consteval block's define_aggregate completes S::Incomplete, and a
// point in a complete-class context of S (a member function body, a
// default member initializer of a nested class) sees the completed type,
// since those are processed after S's closing brace, from which the
// consteval block is reachable. The incomplete-outside-such-a-context
// half is p4143-consteval-block-incomplete-outside-ccc.cpp. GCC 16.1
// agrees; clang does not implement P2996 (spec/oven.md "Manifestly
// constant-evaluated context", P4143R0).
//@ expect: compile-ok
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
#include <meta>
class S {
  class Incomplete;
  class Inner {
    void fn() {
      /* p1 */ Incomplete i; // OK: complete-class context of S
      (void)i;
    }
    Incomplete *p = nullptr;       // a pointer needs no complete type
    int k = sizeof(Incomplete);    // OK: complete-class context of S
  }; /* p2 */
  consteval {
    std::meta::define_aggregate(^^Incomplete, {});
  }
}; /* p3 */
static_assert(sizeof(S) > 0);
