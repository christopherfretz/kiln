// [expr.const.const]/3 (P4101R1): an expression that odr-uses a variable
// declaring an immediate object must be in an immediate function
// context; the diagnostic is required when the use and the variable's
// definition are reachable from one another, which within one
// translation unit they always are. Here the uses precede the definition:
// at the use `s` is not yet known to be an immediate object, so kiln
// remembers the use and reports it, with the declaration, once the
// definition's value is known. A use of `t`, whose value is the null
// reflection, is fine. Compared with GCC 16.1 (which rejects the
// declaration of `s` itself under the P2996R13 type rule).
//@ expect: compile-fail
//@ oracle: gcc
//@ args: -freflection
//@ skip(clang): clang 19/22 do not implement P2996 reflection
//@ strict-diags
//@ diag: 24:15 error "consteval-only value outside an immediate function context"
//@ diag: 26:13 note "declared here"
//@ diag: 25:20 error "consteval-only value outside an immediate function context"
//@ diag: 26:13 note "declared here"
using info = decltype(^^int);
struct S { info r; int n; };
extern const S s;
extern const S t;
const S* p = &s;                 // error, once `s` is defined
int use() { return s.n + t.n; }  // error for `s` only
constexpr S s{^^int, 1};
constexpr S t{{}, 2};
